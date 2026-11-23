use crate::storage::{self, StorageMode, StoredFile};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Bibliography {
    pub source: String,
    pub identifiers: BTreeMap<String, String>,
    pub cover: Option<StoredFile>,
    #[serde(default)]
    pub fields: BTreeMap<String, Vec<String>>,
}

/// Covers are copied as library resources, even when the ebook itself stays linked.
pub fn preserve_cover(source: &Path, root: &Path, id: u64) -> Result<StoredFile, String> {
    let hash = storage::hash_file(source)?;
    let extension = source
        .extension()
        .and_then(|s| s.to_str())
        .filter(|s| s.chars().all(|c| c.is_ascii_alphanumeric()))
        .unwrap_or("image");
    let relative = PathBuf::from("covers")
        .join(id.to_string())
        .join(format!("{hash}.{extension}"));
    let target = storage::safe_join(root, &relative)?;
    std::fs::create_dir_all(target.parent().ok_or("Portada sin carpeta")?)
        .map_err(|e| e.to_string())?;
    if target.exists() {
        if storage::hash_file(&target)? != hash {
            return Err("La portada existente ha cambiado; no se sustituirá".into());
        }
    } else if storage::copy_new(source, &target)? != hash {
        return Err("La portada cambió durante la copia".into());
    }
    Ok(StoredFile {
        path: relative,
        mode: StorageMode::Managed,
        sha256: hash,
    })
}

pub struct Embedded {
    pub metadata: crate::import_jobs::Metadata,
    pub cover: Option<(Vec<u8>, String)>,
    pub warning: Option<String>,
}

fn zip_bytes(
    archive: &mut zip::ZipArchive<std::fs::File>,
    name: &str,
    limit: u64,
) -> Result<Vec<u8>, String> {
    use std::io::Read;
    let file = archive
        .by_name(name)
        .map_err(|e| format!("EPUB: {name}: {e}"))?;
    if file.size() > limit {
        return Err(format!("EPUB: recurso demasiado grande: {name}"));
    }
    let mut bytes = Vec::new();
    file.take(limit + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() as u64 > limit {
        return Err("Recurso EPUB excede el límite".into());
    }
    Ok(bytes)
}

fn attr(
    element: &quick_xml::events::BytesStart<'_>,
    name: &[u8],
) -> Result<Option<String>, String> {
    for attribute in element.attributes() {
        let attribute = attribute.map_err(|e| e.to_string())?;
        if attribute.key.local_name().as_ref() == name {
            return attribute
                .normalized_value(quick_xml::XmlVersion::Implicit1_0)
                .map(|v| Some(v.into_owned()))
                .map_err(|e| e.to_string());
        }
    }
    Ok(None)
}

/// Resolve an EPUB URI inside the ZIP. Never extract arbitrary archive paths to disk.
fn archive_path(base: &str, href: &str) -> Result<String, String> {
    let href = href.split('#').next().unwrap_or(href);
    let mut decoded = Vec::new();
    let mut bytes = href.as_bytes().iter().copied();
    while let Some(byte) = bytes.next() {
        if byte == b'%' {
            let hi = bytes
                .next()
                .and_then(|b| (b as char).to_digit(16))
                .ok_or("URI EPUB inválida")?;
            let lo = bytes
                .next()
                .and_then(|b| (b as char).to_digit(16))
                .ok_or("URI EPUB inválida")?;
            decoded.push((hi * 16 + lo) as u8);
        } else {
            decoded.push(byte);
        }
    }
    let href = String::from_utf8(decoded).map_err(|e| e.to_string())?;
    if href.starts_with('/') || href.contains(['\\', ':', '\0']) {
        return Err("Ruta EPUB fuera del contenedor".into());
    }
    let mut parts: Vec<_> = base.split('/').filter(|p| !p.is_empty()).collect();
    for part in href.split('/') {
        match part {
            "" | "." => (),
            ".." => {
                parts.pop().ok_or("Ruta EPUB fuera del contenedor")?;
            }
            _ => parts.push(part),
        }
    }
    if parts.is_empty() {
        return Err("Ruta EPUB vacía".into());
    }
    Ok(parts.join("/"))
}

pub fn read_epub(path: &Path) -> Result<Embedded, String> {
    use quick_xml::events::Event;
    let mut archive = zip::ZipArchive::new(std::fs::File::open(path).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;
    let container = zip_bytes(&mut archive, "META-INF/container.xml", 1024 * 1024)?;
    let mut reader = quick_xml::Reader::from_reader(container.as_slice());
    let mut package = None;
    loop {
        match reader.read_event().map_err(|e| e.to_string())? {
            Event::Start(e) | Event::Empty(e) if e.local_name().as_ref() == b"rootfile" => {
                if package.is_none() {
                    package = attr(&e, b"full-path")?;
                }
            }
            Event::Eof => break,
            _ => (),
        }
    }
    let package = archive_path("", &package.ok_or("EPUB sin paquete OPF")?)?;
    let opf = zip_bytes(&mut archive, &package, 16 * 1024 * 1024)?;
    let mut reader = quick_xml::Reader::from_reader(opf.as_slice());
    reader.config_mut().expand_empty_elements = true;
    let mut fields: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut identifiers = BTreeMap::new();
    let mut current: Option<(String, String, String)> = None;
    let mut in_metadata = false;
    let mut cover_id = None;
    let mut cover_href = None;
    let mut items = BTreeMap::new();
    loop {
        match reader.read_event().map_err(|e| e.to_string())? {
            Event::Start(e) | Event::Empty(e) => {
                let name = String::from_utf8_lossy(e.local_name().as_ref()).into_owned();
                if name == "metadata" {
                    in_metadata = true;
                }
                if in_metadata
                    && current.is_none()
                    && [
                        "title",
                        "creator",
                        "date",
                        "description",
                        "subject",
                        "identifier",
                        "publisher",
                        "language",
                        "contributor",
                        "rights",
                        "relation",
                        "source",
                        "coverage",
                        "type",
                        "format",
                    ]
                    .contains(&name.as_str())
                {
                    let key = attr(&e, b"scheme")?
                        .or(attr(&e, b"id")?)
                        .unwrap_or_else(|| format!("identifier-{}", identifiers.len() + 1));
                    current = Some((name.clone(), key, String::new()));
                }
                if name == "meta" && attr(&e, b"name")?.as_deref() == Some("cover") {
                    cover_id = attr(&e, b"content")?;
                }
                if name == "item" {
                    if let (Some(id), Some(href)) = (attr(&e, b"id")?, attr(&e, b"href")?) {
                        if attr(&e, b"properties")?
                            .is_some_and(|p| p.split_whitespace().any(|p| p == "cover-image"))
                        {
                            cover_href = Some(href.clone());
                        }
                        items.insert(id, href);
                    }
                }
            }
            Event::Text(e) => {
                if let Some((_, _, text)) = &mut current {
                    text.push_str(&e.decode().map_err(|e| e.to_string())?);
                }
            }
            Event::CData(e) => {
                if let Some((_, _, text)) = &mut current {
                    text.push_str(&e.decode().map_err(|e| e.to_string())?);
                }
            }
            Event::GeneralRef(e) => {
                if let Some((_, _, text)) = &mut current {
                    let entity = format!("&{};", e.decode().map_err(|e| e.to_string())?);
                    text.push_str(
                        &quick_xml::escape::unescape(&entity).map_err(|e| e.to_string())?,
                    );
                }
            }
            Event::End(e) => {
                let name = String::from_utf8_lossy(e.local_name().as_ref()).into_owned();
                if current.as_ref().is_some_and(|(n, _, _)| n == &name) {
                    let (name, key, value) = current.take().unwrap();
                    if !value.trim().is_empty() {
                        if name == "identifier" {
                            identifiers.insert(key, value.trim().into());
                        } else {
                            fields.entry(name).or_default().push(value.trim().into());
                        }
                    }
                }
                if name == "metadata" {
                    in_metadata = false;
                }
            }
            Event::Eof => break,
            _ => (),
        }
    }
    let first = |name: &str| {
        fields
            .get(name)
            .and_then(|v| v.first())
            .cloned()
            .unwrap_or_default()
    };
    let metadata = crate::import_jobs::Metadata {
        title: first("title"),
        author: fields
            .get("creator")
            .map(|v| v.join(" & "))
            .unwrap_or_default(),
        year: first("date")
            .get(..4)
            .filter(|s| s.chars().all(|c| c.is_ascii_digit()))
            .unwrap_or("")
            .into(),
        description: first("description"),
        tags: fields.get("subject").cloned().unwrap_or_default(),
        bibliography: Bibliography {
            source: "Metadatos incrustados (EPUB/OPF)".into(),
            identifiers,
            cover: None,
            fields,
        },
        cover_source: None,
    };
    let mut warning = None;
    let cover = if let Some(href) =
        cover_href.or_else(|| cover_id.and_then(|id| items.get(&id).cloned()))
    {
        let read = (|| {
            let base = package.rsplit_once('/').map(|(base, _)| base).unwrap_or("");
            let name = archive_path(base, &href)?;
            let bytes = zip_bytes(&mut archive, &name, 32 * 1024 * 1024)?;
            let extension = name
                .rsplit_once('.')
                .map(|(_, ext)| ext)
                .unwrap_or("image")
                .to_owned();
            Ok::<_, String>((bytes, extension))
        })();
        match read {
            Ok(cover) => Some(cover),
            Err(e) => {
                warning = Some(format!("Portada EPUB no disponible: {e}"));
                None
            }
        }
    } else {
        None
    };
    Ok(Embedded {
        metadata,
        cover,
        warning,
    })
}

pub fn preserve_cover_bytes(
    bytes: &[u8],
    extension: &str,
    root: &Path,
    id: u64,
) -> Result<StoredFile, String> {
    use std::io::Write;
    let extension = if extension.chars().all(|c| c.is_ascii_alphanumeric()) {
        extension
    } else {
        "image"
    };
    let mut temporary = tempfile::Builder::new()
        .suffix(&format!(".{extension}"))
        .tempfile()
        .map_err(|e| e.to_string())?;
    temporary.write_all(bytes).map_err(|e| e.to_string())?;
    preserve_cover(temporary.path(), root, id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    fn epub(folder: &Path, cover_href: &str) -> PathBuf {
        let path = folder.join("filename.epub");
        let mut zip = zip::ZipWriter::new(std::fs::File::create(&path).unwrap());
        let options = zip::write::SimpleFileOptions::default();
        zip.start_file("META-INF/container.xml", options).unwrap();
        zip.write_all(br#"<?xml version='1.0'?><container><rootfiles><rootfile full-path = 'OPS/package.opf' /></rootfiles></container>"#).unwrap();
        zip.start_file("OPS/package.opf", options).unwrap();
        write!(zip,"<package xmlns:dc='http://purl.org/dc/elements/1.1/' xmlns:opf='http://www.idpf.org/2007/opf'><metadata><dc:title>Título &amp; otro</dc:title><dc:creator>Autora</dc:creator><dc:creator>Autor</dc:creator><dc:date>2021-02-03</dc:date><dc:description><![CDATA[Descripción original]]></dc:description><dc:subject>Historia</dc:subject><dc:identifier opf:scheme='ISBN'>12345</dc:identifier><meta name='cover' content='cover-id'/></metadata><manifest><item id='cover-id' href='{cover_href}' properties='cover-image' media-type='image/png'/></manifest></package>").unwrap();
        zip.start_file("OPS/images/cover a.png", options).unwrap();
        zip.write_all(b"cover bytes").unwrap();
        zip.finish().unwrap();
        path
    }
    #[test]
    fn epub_import_preserves_embedded_metadata_entities_identifiers_and_cover_without_calibre() {
        let source = tempfile::tempdir().unwrap();
        let root = tempfile::tempdir().unwrap();
        let path = epub(source.path(), "images/cover%20a.png");
        let before = storage::hash_file(&path).unwrap();
        let request = crate::import_jobs::ImportRequest {
            book_id: 42,
            associate: false,
            path: path.clone(),
            source_base: None,
            metadata: None,
        };
        let prepared = crate::import_jobs::prepare(
            &request,
            root.path(),
            &storage::Preferences::default(),
            &|_| panic!("EPUB does not require Calibre"),
        )
        .unwrap();
        assert_eq!(prepared.metadata.title, "Título & otro");
        assert_eq!(prepared.metadata.author, "Autora & Autor");
        assert_eq!(prepared.metadata.year, "2021");
        assert_eq!(prepared.metadata.description, "Descripción original");
        assert_eq!(prepared.metadata.tags, vec!["Historia"]);
        assert_eq!(prepared.metadata.bibliography.identifiers["ISBN"], "12345");
        assert!(prepared.warning.is_none());
        assert_eq!(
            std::fs::read(
                prepared
                    .metadata
                    .bibliography
                    .cover
                    .unwrap()
                    .resolve(root.path())
                    .unwrap()
            )
            .unwrap(),
            b"cover bytes"
        );
        assert_eq!(storage::hash_file(&path).unwrap(), before);
    }
    #[test]
    fn epub_cover_traversal_is_reported_without_discarding_metadata() {
        let source = tempfile::tempdir().unwrap();
        let path = epub(source.path(), "../../outside.png");
        let embedded = read_epub(&path).unwrap();
        assert_eq!(embedded.metadata.title, "Título & otro");
        assert!(embedded.cover.is_none());
        assert!(embedded.warning.is_some());
        assert!(archive_path("OPS", "https://example.com/cover.png").is_err());
        assert!(archive_path("OPS", "%2foutside.png").is_err());
        assert_eq!(
            archive_path("OPS/text", "../images/a.png").unwrap(),
            "OPS/images/a.png"
        );
    }
}
