//! Managed books use relative paths; linked books remain untouched at their origin.
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File},
    io::{Read, Write},
    path::{Component, Path, PathBuf},
};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum StorageMode {
    #[default]
    Linked,
    Managed,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Layout {
    #[default]
    Preserve,
    AuthorTitle,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Preferences {
    pub import_mode: StorageMode,
    pub layout: Layout,
    pub include_external_in_backup: bool,
}

impl Preferences {
    pub fn reset_preserving(root: &Path) -> Result<Option<PathBuf>, String> {
        let original = match fs::read(root.join("library-settings.json")) {
            Ok(bytes) => Some(bytes),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
            Err(e) => return Err(e.to_string()),
        };
        let preserved = if let Some(bytes) = original {
            let state = safe_join(root, Path::new("state"))?;
            fs::create_dir_all(&state).map_err(|e| e.to_string())?;
            let mut file = tempfile::Builder::new()
                .prefix("settings-recovery-")
                .suffix(".json")
                .tempfile_in(state)
                .map_err(|e| e.to_string())?;
            file.write_all(&bytes).map_err(|e| e.to_string())?;
            file.as_file().sync_all().map_err(|e| e.to_string())?;
            Some(file.keep().map_err(|e| e.to_string())?.1)
        } else {
            None
        };
        Self::default().save(root)?;
        Ok(preserved)
    }
    pub fn load(root: &Path) -> Result<Self, String> {
        match fs::read(root.join("library-settings.json")) {
            Ok(bytes) => {
                serde_json::from_slice(&bytes).map_err(|e| format!("Preferencias ilegibles: {e}"))
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(e) => Err(e.to_string()),
        }
    }
    pub fn save(&self, root: &Path) -> Result<(), String> {
        crate::persistence::atomic_write(
            &root.join("library-settings.json"),
            &serde_json::to_vec_pretty(self).map_err(|e| e.to_string())?,
        )
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StoredFile {
    pub path: PathBuf,
    pub mode: StorageMode,
    pub sha256: String,
}

impl StoredFile {
    pub fn resolve(&self, root: &Path) -> Result<PathBuf, String> {
        match self.mode {
            StorageMode::Linked if self.path.is_absolute() => Ok(self.path.clone()),
            StorageMode::Linked => Err("Un archivo vinculado requiere una ruta absoluta".into()),
            StorageMode::Managed => safe_join(root, &self.path),
        }
    }
}

pub fn hash_file(path: &Path) -> Result<String, String> {
    let mut file = File::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut digest = Sha256::new();
    let mut buffer = [0u8; 65536];
    loop {
        let count = file.read(&mut buffer).map_err(|e| e.to_string())?;
        if count == 0 {
            break;
        }
        digest.update(&buffer[..count]);
    }
    Ok(format!("{:x}", digest.finalize()))
}

pub fn safe_join(root: &Path, relative: &Path) -> Result<PathBuf, String> {
    if relative.as_os_str().is_empty()
        || relative
            .components()
            .any(|c| !matches!(c, Component::Normal(_)))
    {
        return Err("Ruta administrada inválida; debe ser relativa y no contener '..'".into());
    }
    // Reject alternate streams and cross-platform backslash traversal in imported manifests.
    for component in relative.components() {
        let name = component.as_os_str().to_string_lossy();
        if name.contains([':', '\\']) || name.ends_with(['.', ' ']) {
            return Err("Ruta administrada no portable".into());
        }
    }
    let mut path = root.to_owned();
    for component in relative.components() {
        path.push(component);
        match fs::symlink_metadata(&path) {
            Ok(meta) if meta.file_type().is_symlink() => {
                return Err("No se permiten enlaces simbólicos dentro del almacén".into())
            }
            Ok(_) => {
                let canonical_root = fs::canonicalize(root).map_err(|e| e.to_string())?;
                let canonical = fs::canonicalize(&path).map_err(|e| e.to_string())?;
                if !canonical.starts_with(canonical_root) {
                    return Err("La ruta sale de la biblioteca".into());
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => (),
            Err(e) => return Err(e.to_string()),
        }
    }
    Ok(path)
}

fn safe_name(value: &str) -> String {
    let name: String = value
        .chars()
        .take(64)
        .map(|c| {
            if c.is_control() || "<>:\"/\\|?*".contains(c) {
                '_'
            } else {
                c
            }
        })
        .collect();
    let name = name.trim().trim_end_matches(['.', ' ']);
    // A prefix makes even Windows reserved names safe.
    format!("_{}", if name.is_empty() { "sin-datos" } else { name })
}

/// Copy, sync and hash-verify before publishing; never replace an existing file.
pub fn copy_new(source: &Path, destination: &Path) -> Result<String, String> {
    let before = hash_file(source)?;
    let parent = destination.parent().ok_or("Destino sin carpeta")?;
    let mut staged = tempfile::NamedTempFile::new_in(parent).map_err(|e| e.to_string())?;
    std::io::copy(
        &mut File::open(source).map_err(|e| e.to_string())?,
        &mut staged,
    )
    .map_err(|e| e.to_string())?;
    staged.flush().map_err(|e| e.to_string())?;
    staged.as_file().sync_all().map_err(|e| e.to_string())?;
    if hash_file(staged.path())? != before || hash_file(source)? != before {
        return Err("El archivo cambió durante la copia o la copia no es íntegra".into());
    }
    staged
        .persist_noclobber(destination)
        .map_err(|e| e.to_string())?;
    Ok(before)
}

#[allow(clippy::too_many_arguments)]
pub fn import_file(
    source: &Path,
    root: &Path,
    id: u64,
    author: &str,
    title: &str,
    mode: StorageMode,
    layout: Layout,
    source_base: Option<&Path>,
) -> Result<StoredFile, String> {
    if !root.is_dir() {
        return Err("La carpeta de biblioteca no está disponible".into());
    }
    let source = fs::canonicalize(source).map_err(|e| e.to_string())?;
    if !source.is_file() {
        return Err("El origen no es un archivo".into());
    }
    if mode == StorageMode::Linked {
        return Ok(StoredFile {
            sha256: hash_file(&source)?,
            path: source,
            mode,
        });
    }
    let name = source.file_name().ok_or("Archivo sin nombre")?;
    let relative = match layout {
        Layout::Preserve => {
            let base = source_base
                .map(fs::canonicalize)
                .transpose()
                .map_err(|e| e.to_string())?;
            let subtree = match base.as_ref() {
                Some(base) => source
                    .strip_prefix(base)
                    .map_err(|_| "El archivo no pertenece a la carpeta de origen")?,
                None => Path::new(name),
            };
            PathBuf::from("books").join(id.to_string()).join(subtree)
        }
        Layout::AuthorTitle => PathBuf::from("books")
            .join(safe_name(author))
            .join(format!("{}-{id}", safe_name(title)))
            .join(name),
    };
    let output = safe_join(root, &relative)?;
    let parent = output.parent().ok_or("Destino sin carpeta")?;
    fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    // Recheck after creating parents; never follow an existing junction out of root.
    safe_join(root, &relative)?;
    let hash = hash_file(&source)?;
    for number in 1..10000 {
        let candidate = if number == 1 {
            output.clone()
        } else {
            output.with_file_name(format!(
                "{} ({number}).{}",
                output.file_stem().unwrap_or_default().to_string_lossy(),
                output.extension().unwrap_or_default().to_string_lossy()
            ))
        };
        if candidate.exists() {
            if candidate.is_file() && hash_file(&candidate)? == hash {
                return Ok(StoredFile {
                    path: candidate
                        .strip_prefix(root)
                        .map_err(|e| e.to_string())?
                        .to_owned(),
                    mode,
                    sha256: hash,
                });
            }
            continue;
        }
        match copy_new(&source, &candidate) {
            Ok(sha256) => {
                return Ok(StoredFile {
                    path: candidate
                        .strip_prefix(root)
                        .map_err(|e| e.to_string())?
                        .to_owned(),
                    mode,
                    sha256,
                })
            }
            Err(_) if candidate.exists() => continue,
            Err(error) => return Err(error),
        }
    }
    Err("Demasiados archivos con ese nombre".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn resetting_invalid_preferences_keeps_original_bytes_for_recovery() {
        let root = tempfile::tempdir().unwrap();
        fs::write(
            root.path().join("library-settings.json"),
            b"corrupt settings",
        )
        .unwrap();
        assert!(Preferences::load(root.path()).is_err());
        let preserved = Preferences::reset_preserving(root.path()).unwrap().unwrap();
        assert_eq!(fs::read(preserved).unwrap(), b"corrupt settings");
        assert_eq!(
            Preferences::load(root.path()).unwrap().import_mode,
            StorageMode::Linked
        );
    }
    #[test]
    fn linked_and_both_managed_layouts_preserve_source_and_resolve_after_move() {
        let origin = tempfile::tempdir().unwrap();
        let library = tempfile::tempdir().unwrap();
        fs::create_dir(origin.path().join("serie")).unwrap();
        let source = origin.path().join("serie/book.txt");
        fs::write(&source, b"contenido").unwrap();
        let linked = import_file(
            &source,
            library.path(),
            1,
            "Autor",
            "Título",
            StorageMode::Linked,
            Layout::Preserve,
            None,
        )
        .unwrap();
        assert_eq!(
            linked.resolve(library.path()).unwrap(),
            fs::canonicalize(&source).unwrap()
        );
        for layout in [Layout::Preserve, Layout::AuthorTitle] {
            let file = import_file(
                &source,
                library.path(),
                2,
                "CON:autor",
                "Título",
                StorageMode::Managed,
                layout,
                Some(origin.path()),
            )
            .unwrap();
            assert!(!file.path.is_absolute());
            assert_eq!(
                fs::read(file.resolve(library.path()).unwrap()).unwrap(),
                b"contenido"
            );
            let moved = tempfile::tempdir().unwrap();
            let new_path = file.resolve(moved.path()).unwrap();
            fs::create_dir_all(new_path.parent().unwrap()).unwrap();
            fs::copy(file.resolve(library.path()).unwrap(), &new_path).unwrap();
            assert_eq!(hash_file(&new_path).unwrap(), file.sha256);
        }
        assert_eq!(fs::read(source).unwrap(), b"contenido");
    }
    #[test]
    fn collisions_never_replace_existing_content_and_same_content_is_reused() {
        let origin = tempfile::tempdir().unwrap();
        let library = tempfile::tempdir().unwrap();
        let source = origin.path().join("book.txt");
        fs::write(&source, b"first").unwrap();
        let import = || {
            import_file(
                &source,
                library.path(),
                1,
                "A",
                "T",
                StorageMode::Managed,
                Layout::Preserve,
                None,
            )
            .unwrap()
        };
        let first = import();
        assert_eq!(first.path, import().path);
        fs::write(&source, b"second").unwrap();
        let second = import();
        assert_ne!(first.path, second.path);
        assert_eq!(
            fs::read(first.resolve(library.path()).unwrap()).unwrap(),
            b"first"
        );
    }
    #[test]
    fn unsafe_paths_and_missing_origins_fail_without_writes() {
        let dir = tempfile::tempdir().unwrap();
        for path in [
            "../escape",
            "/absolute",
            "books/../../x",
            "books/x:stream",
            "books/..\\escape",
        ] {
            assert!(safe_join(dir.path(), Path::new(path)).is_err());
        }
        assert!(import_file(
            Path::new("nonexistent"),
            dir.path(),
            1,
            "A",
            "T",
            StorageMode::Managed,
            Layout::Preserve,
            None
        )
        .is_err());
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 0);
    }
    #[cfg(unix)]
    #[test]
    fn symlink_cannot_escape_library() {
        let dir = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        std::os::unix::fs::symlink(outside.path(), dir.path().join("books")).unwrap();
        assert!(safe_join(dir.path(), Path::new("books/file.txt")).is_err());
    }
}
