//! Stage, validate, then publish without replacing an existing device file.
use std::{
    fs::{self, File},
    io::{Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
    process::Command,
};

pub fn output_format<'a>(source_format: &'a str, device_name: &str, preferred: &'a str) -> &'a str {
    let compatible = if device_name.starts_with("Kindle") {
        ["azw3", "mobi", "pdf", "txt"].contains(&source_format)
    } else {
        [preferred, "pdf", "txt"].contains(&source_format)
    };
    if compatible {
        source_format
    } else {
        preferred
    }
}

// Signature validation detects renamed files and empty/obviously invalid converter output.
// It does not certify rendering or device firmware compatibility.
pub fn validate(path: &Path, format: &str) -> Result<(), String> {
    let mut file = File::open(path).map_err(|e| e.to_string())?;
    if file.metadata().map_err(|e| e.to_string())?.len() == 0 {
        return Err("El archivo está vacío".into());
    }
    let mut header = [0u8; 80];
    let read = file.read(&mut header).map_err(|e| e.to_string())?;
    let valid = match format {
        "pdf" => header[..read].starts_with(b"%PDF-"),
        "epub" => {
            let mut archive =
                zip::ZipArchive::new(file).map_err(|e| format!("EPUB inválido: {e}"))?;
            let mut mime = String::new();
            archive
                .by_name("mimetype")
                .map_err(|e| e.to_string())?
                .take(128)
                .read_to_string(&mut mime)
                .map_err(|e| e.to_string())?;
            mime.trim() == "application/epub+zip"
                && archive.by_name("META-INF/container.xml").is_ok()
        }
        "mobi" | "azw3" => read >= 68 && &header[60..68] == b"BOOKMOBI",
        "txt" => {
            !header[..read].contains(&0)
                || header[..read].starts_with(&[0xff, 0xfe])
                || header[..read].starts_with(&[0xfe, 0xff])
        }
        _ => true,
    };
    if valid {
        Ok(())
    } else {
        Err(format!("El contenido no corresponde al formato {format}"))
    }
}

fn same_bytes(first: &Path, second: &Path) -> Result<bool, String> {
    let mut a = File::open(first).map_err(|e| e.to_string())?;
    let mut b = File::open(second).map_err(|e| e.to_string())?;
    if a.metadata().map_err(|e| e.to_string())?.len()
        != b.metadata().map_err(|e| e.to_string())?.len()
    {
        return Ok(false);
    }
    let mut left = [0u8; 65536];
    let mut right = [0u8; 65536];
    loop {
        let count = a.read(&mut left).map_err(|e| e.to_string())?;
        if count == 0 {
            return Ok(true);
        }
        b.read_exact(&mut right[..count])
            .map_err(|e| e.to_string())?;
        if left[..count] != right[..count] {
            return Ok(false);
        }
    }
}

fn safe_stem(title: &str) -> String {
    let name: String = title
        .chars()
        .take(80)
        .map(|c| {
            if "<>:\"/\\|?*".contains(c) || c.is_control() {
                '_'
            } else {
                c
            }
        })
        .collect();
    let name = name.trim().trim_end_matches(['.', ' ']);
    // Prefix also avoids Windows reserved device basenames, including CON, AUX, COM1.
    format!(
        "Libro-{}",
        if name.is_empty() { "sin-título" } else { name }
    )
}

pub fn send(
    source: &Path,
    title: &str,
    device_name: &str,
    preferred: &str,
    destination: &Path,
    converter: Option<&Path>,
) -> Result<PathBuf, String> {
    if !source.is_file() {
        return Err("No se encontró el archivo original".into());
    }
    let current = source
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    let format = output_format(&current, device_name, preferred);
    // Do not recreate a vanished drive/mount path after disconnection.
    if !destination.parent().is_some_and(Path::is_dir) {
        return Err("El lector ya no está disponible".into());
    }
    fs::create_dir_all(destination).map_err(|e| e.to_string())?;
    publish(source, title, format, destination, converter)
}

pub fn convert_file(
    source: &Path,
    format: &str,
    converter: Option<&Path>,
) -> Result<PathBuf, String> {
    if !["epub", "mobi", "azw3", "pdf", "txt", "html", "rtf", "fb2"].contains(&format) {
        return Err("Formato de salida no admitido".into());
    }
    let source = fs::canonicalize(source).map_err(|e| e.to_string())?;
    let destination = source.parent().ok_or("El origen no tiene carpeta")?;
    publish(
        &source,
        source
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("Libro"),
        format,
        destination,
        converter,
    )
}

fn publish(
    source: &Path,
    title: &str,
    format: &str,
    destination: &Path,
    converter: Option<&Path>,
) -> Result<PathBuf, String> {
    if !source.is_file() {
        return Err("No se encontró el archivo original".into());
    }
    let current = source
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    let staging = tempfile::tempdir_in(destination).map_err(|e| e.to_string())?;
    let converted = staging.path().join(format!("conversion.{format}"));
    let input = if current == format {
        validate(source, format)?;
        source
    } else {
        let converter =
            converter.ok_or("Calibre no está instalado; se requiere conversión real")?;
        let result = Command::new(converter)
            .arg(source)
            .arg(&converted)
            .output()
            .map_err(|e| e.to_string())?;
        if !result.status.success() {
            return Err(format!(
                "ebook-convert terminó con {}: {}",
                result.status,
                String::from_utf8_lossy(&result.stderr)
                    .lines()
                    .next()
                    .unwrap_or("sin detalle")
            ));
        }
        converted.as_path()
    };
    validate(input, format)?;
    let mut staged = tempfile::NamedTempFile::new_in(destination).map_err(|e| e.to_string())?;
    std::io::copy(
        &mut File::open(input).map_err(|e| e.to_string())?,
        &mut staged,
    )
    .map_err(|e| e.to_string())?;
    staged.flush().map_err(|e| e.to_string())?;
    staged.as_file().sync_all().map_err(|e| e.to_string())?;
    staged
        .as_file_mut()
        .seek(SeekFrom::Start(0))
        .map_err(|e| e.to_string())?;
    if !same_bytes(input, staged.path())? {
        return Err("La copia no coincide con el archivo de origen".into());
    }
    validate(staged.path(), format)?;
    let stem = safe_stem(title);
    for number in 1..10000 {
        let suffix = if number == 1 {
            String::new()
        } else {
            format!(" ({number})")
        };
        let output = destination.join(format!("{stem}{suffix}.{format}"));
        match staged.persist_noclobber(&output) {
            Ok(_) => return Ok(output),
            Err(error) if error.error.kind() == std::io::ErrorKind::AlreadyExists => {
                staged = error.file
            }
            Err(error) => return Err(error.error.to_string()),
        }
    }
    Err("Demasiados archivos con el mismo nombre; cambie el título".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn manual_conversion_requires_requested_format_and_preserves_input() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("source.txt");
        fs::write(&source, b"text").unwrap();
        assert!(convert_file(&source, "pdf", None).is_err());
        assert!(convert_file(&source, "../pdf", None).is_err());
        assert_eq!(fs::read(&source).unwrap(), b"text");
        let copy = convert_file(&source, "txt", None).unwrap();
        assert_ne!(copy, source);
        assert_eq!(fs::read(copy).unwrap(), b"text");
    }
    #[cfg(unix)]
    #[test]
    fn manual_conversion_publishes_validated_output_without_replacing_previous_conversion() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("source.txt");
        fs::write(&source, b"text").unwrap();
        let converter = dir.path().join("converter");
        fs::write(
            &converter,
            "#!/bin/sh\nprintf '%%PDF-1.4\\nfixture\\n' > \"$2\"\n",
        )
        .unwrap();
        fs::set_permissions(&converter, fs::Permissions::from_mode(0o700)).unwrap();
        let first = convert_file(&source, "pdf", Some(&converter)).unwrap();
        let second = convert_file(&source, "pdf", Some(&converter)).unwrap();
        assert_ne!(first, second);
        assert_eq!(first.extension().unwrap(), "pdf");
        assert_eq!(fs::read(first).unwrap(), fs::read(second).unwrap());
        assert_eq!(fs::read(source).unwrap(), b"text");
    }
    #[test]
    fn epub_container_and_mobi_signature_are_checked_before_copying() {
        let dir = tempfile::tempdir().unwrap();
        let epub = dir.path().join("source.epub");
        let mut archive = zip::ZipWriter::new(File::create(&epub).unwrap());
        let options = zip::write::SimpleFileOptions::default();
        archive.start_file("mimetype", options).unwrap();
        archive.write_all(b"application/epub+zip").unwrap();
        archive
            .start_file("META-INF/container.xml", options)
            .unwrap();
        archive.write_all(b"<container/>").unwrap();
        archive.finish().unwrap();
        let output = send(
            &epub,
            "EPUB",
            "Kobo",
            "epub",
            &dir.path().join("Documents"),
            None,
        )
        .unwrap();
        assert_eq!(fs::read(output).unwrap(), fs::read(&epub).unwrap());
        let mobi = dir.path().join("source.mobi");
        let mut bytes = vec![0; 100];
        bytes[60..68].copy_from_slice(b"BOOKMOBI");
        fs::write(&mobi, &bytes).unwrap();
        let output = send(
            &mobi,
            "MOBI",
            "Kindle",
            "azw3",
            &dir.path().join("Documents"),
            None,
        )
        .unwrap();
        assert_eq!(output.extension().unwrap(), "mobi");
        assert_eq!(fs::read(output).unwrap(), bytes);
        fs::write(epub, b"PK-not-an-epub").unwrap();
        assert!(validate(&dir.path().join("source.epub"), "epub").is_err());
    }
    #[test]
    fn copies_preserve_format_bytes_and_existing_files() {
        let dir = tempfile::tempdir().unwrap();
        for device in ["Kindle", "Kobo", "NOOK"] {
            let source = dir.path().join("source.PDF");
            fs::write(&source, b"%PDF-1.4\nfixture\n%%EOF").unwrap();
            let dest = dir.path().join(device);
            let preferred = if device == "Kindle" { "azw3" } else { "epub" };
            let first = send(&source, "CON: prueba", device, preferred, &dest, None).unwrap();
            let second = send(&source, "CON: prueba", device, preferred, &dest, None).unwrap();
            assert_ne!(first, second);
            assert_eq!(first.extension().unwrap(), "pdf");
            assert_eq!(fs::read(first).unwrap(), fs::read(&source).unwrap());
            assert_eq!(fs::read(second).unwrap(), fs::read(source).unwrap());
        }
        assert_eq!(output_format("mobi", "Kindle", "azw3"), "mobi");
        assert_eq!(output_format("txt", "Kobo", "epub"), "txt");
        assert_eq!(output_format("epub", "Kindle", "azw3"), "azw3");
    }

    #[test]
    fn rejects_renamed_empty_and_missing_sources_without_publishing() {
        let dir = tempfile::tempdir().unwrap();
        let dest = dir.path().join("Documents");
        let source = dir.path().join("fake.pdf");
        for bytes in [b"not a pdf".as_slice(), b""] {
            fs::write(&source, bytes).unwrap();
            assert!(send(&source, "Test", "Kindle", "azw3", &dest, None).is_err());
            assert_eq!(fs::read_dir(&dest).unwrap().count(), 0);
        }
        fs::remove_file(&source).unwrap();
        assert!(send(&source, "Test", "Kindle", "azw3", &dest, None).is_err());
    }

    #[test]
    fn conversion_is_required_and_never_simulated_by_renaming() {
        let dir = tempfile::tempdir().unwrap();
        let dest = dir.path().join("Documents");
        let source = dir.path().join("source.epub");
        fs::write(&source, b"source preserved").unwrap();
        assert!(send(&source, "Test", "Kindle", "azw3", &dest, None).is_err());
        assert_eq!(fs::read_dir(dest).unwrap().count(), 0);
        assert_eq!(fs::read(source).unwrap(), b"source preserved");
    }

    #[cfg(unix)]
    #[test]
    fn failing_or_lying_converter_cannot_publish_or_remove_existing_files() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let dest = dir.path().join("Documents");
        fs::create_dir(&dest).unwrap();
        let existing = dest.join("Libro-Test.azw3");
        fs::write(&existing, b"keep").unwrap();
        let source = dir.path().join("source.epub");
        fs::write(&source, b"source").unwrap();
        let converter = dir.path().join("converter");
        for script in [
            "#!/bin/sh\nprintf partial > \"$2\"\nexit 1\n",
            "#!/bin/sh\nexit 0\n",
            "#!/bin/sh\nprintf invalid > \"$2\"\n",
        ] {
            fs::write(&converter, script).unwrap();
            fs::set_permissions(&converter, fs::Permissions::from_mode(0o700)).unwrap();
            assert!(send(&source, "Test", "Kindle", "azw3", &dest, Some(&converter)).is_err());
            assert_eq!(fs::read(&existing).unwrap(), b"keep");
            assert_eq!(fs::read_dir(&dest).unwrap().count(), 1);
        }
    }
}
