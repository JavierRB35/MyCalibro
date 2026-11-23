//! Catalog persistence. Never substitute demo data for an unreadable catalog.
use crate::Book;
use std::{
    fs::{self, File, OpenOptions},
    io::{self, Write},
    path::{Path, PathBuf},
};

pub struct Catalog {
    pub path: PathBuf,
    // The lock file stays in place; removing it would allow locks on different inodes.
    _lock: File,
    expected: Option<Vec<u8>>,
}

fn read_optional(path: &Path) -> Result<Option<Vec<u8>>, String> {
    match fs::read(path) {
        Ok(bytes) => Ok(Some(bytes)),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(format!("No se pudo leer {}: {e}", path.display())),
    }
}

pub fn decode(bytes: &[u8]) -> Result<Vec<Book>, String> {
    let books: Vec<Book> = serde_json::from_slice(bytes)
        .map_err(|e| format!("Catálogo ilegible: {e}. El original se conserva."))?;
    let mut ids = std::collections::HashSet::new();
    if books.iter().any(|b| !ids.insert(b.id) || b.id == u64::MAX) {
        return Err("Catálogo con identificadores duplicados o agotados; no se modificará".into());
    }
    Ok(books)
}

pub fn atomic_write(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let parent = path.parent().ok_or("El archivo no tiene carpeta")?;
    let mut temp = tempfile::NamedTempFile::new_in(parent).map_err(|e| e.to_string())?;
    temp.write_all(bytes).map_err(|e| e.to_string())?;
    temp.as_file().sync_all().map_err(|e| e.to_string())?;
    temp.persist(path).map_err(|e| e.to_string())?;
    // Windows replacement is handled by tempfile; on Unix also persist the directory entry.
    #[cfg(unix)]
    File::open(parent)
        .and_then(|dir| dir.sync_all())
        .map_err(|e| e.to_string())?;
    Ok(())
}

impl Catalog {
    /// Reserve durably before importing/saving. Failed imports may leave gaps,
    /// but deletion and restart must never reuse an ID for a different book.
    pub fn allocate_id(&mut self, books: &[Book]) -> Result<u64, String> {
        if read_optional(&self.path)? != self.expected {
            return Err(
                "El catálogo cambió externamente; no se puede reservar un identificador".into(),
            );
        }
        let path = self.path.with_file_name("next-book-id.json");
        let minimum = books
            .iter()
            .map(|b| b.id)
            .max()
            .unwrap_or(0)
            .checked_add(1)
            .ok_or("Identificadores agotados")?;
        let saved = match read_optional(&path)? {
            Some(bytes) => serde_json::from_slice::<u64>(&bytes)
                .map_err(|e| format!("Contador de libros ilegible: {e}"))?,
            None => minimum,
        };
        let next = minimum.max(saved);
        let after = next.checked_add(1).ok_or("Identificadores agotados")?;
        atomic_write(
            &path,
            &serde_json::to_vec(&after).map_err(|e| e.to_string())?,
        )?;
        Ok(next)
    }
    pub fn open(folder: &Path) -> Result<(Self, Vec<Book>), String> {
        // Opening a library does not recreate a missing synchronized/external folder.
        if !folder.is_dir() {
            return Err("La carpeta de biblioteca no está disponible".into());
        }
        let lock = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(folder.join(".mycalibro.lock"))
            .map_err(|e| e.to_string())?;
        lock.try_lock().map_err(|e| {
            format!("Biblioteca abierta en otro proceso o bloqueo no disponible: {e}")
        })?;
        let path = folder.join("library.json");
        let expected = read_optional(&path)?;
        let books = match &expected {
            Some(bytes) => decode(bytes)?,
            None if folder.join("library.previous.json").exists() => {
                return Err(
                    "Falta el catálogo pero existe una copia anterior. Recupérela en otra carpeta."
                        .into(),
                );
            }
            None => Vec::new(),
        };
        Ok((
            Self {
                path,
                _lock: lock,
                expected,
            },
            books,
        ))
    }

    pub fn save(&mut self, books: &[Book]) -> Result<(), String> {
        let bytes = serde_json::to_vec_pretty(books).map_err(|e| e.to_string())?;
        decode(&bytes)?;
        if read_optional(&self.path)? != self.expected {
            return Err("El catálogo cambió fuera de MyCalibro. Se bloqueó el guardado para conservar ambas versiones.".into());
        }
        if self.expected.as_ref() == Some(&bytes) {
            return Ok(());
        }
        if let Some(previous) = &self.expected {
            atomic_write(&self.path.with_file_name("library.previous.json"), previous)?;
        }
        atomic_write(&self.path, &bytes)?;
        self.expected = Some(bytes);
        Ok(())
    }
}

/// Recover a catalogue and its managed references into an EMPTY directory.
/// External references stay untouched. Auxiliary state is not a full backup.
pub fn recover_into(source: &Path, destination: &Path) -> Result<(), String> {
    let bytes = fs::read(source).map_err(|e| e.to_string())?;
    let books = decode(&bytes)?;
    recover_books_into(
        &books,
        source.parent().ok_or("Catálogo sin carpeta")?,
        destination,
    )
}

/// Export pending in-memory edits without breaking relative managed references.
/// A failure may leave verified copies in the new folder, but never publishes an
/// incomplete catalogue or removes anything from the source library.
pub fn recover_books_into(
    books: &[Book],
    source_root: &Path,
    destination: &Path,
) -> Result<(), String> {
    use crate::storage::{self, StorageMode};
    decode(&serde_json::to_vec(books).map_err(|e| e.to_string())?)?;
    if fs::read_dir(destination)
        .map_err(|e| e.to_string())?
        .next()
        .is_some()
    {
        return Err("La recuperación requiere una carpeta vacía; no se sobrescribió nada".into());
    }
    let (mut catalog, _) = Catalog::open(destination)?;
    let mut copied = std::collections::HashSet::new();
    for book in books {
        for file in book.files.iter().chain(book.bibliography.cover.iter()) {
            if file.mode != StorageMode::Managed {
                continue;
            }
            let source = file.resolve(source_root)?;
            let target = storage::safe_join(destination, &file.path)?;
            let hash = if copied.insert(file.path.clone()) {
                fs::create_dir_all(target.parent().ok_or("Archivo sin carpeta")?)
                    .map_err(|e| e.to_string())?;
                storage::copy_new(&source, &target)?
            } else {
                storage::hash_file(&target)?
            };
            if !file.sha256.is_empty() && hash != file.sha256 {
                return Err(format!(
                    "El archivo administrado cambió: {}. No se publicó el catálogo recuperado",
                    file.path.display()
                ));
            }
        }
    }
    let counter_path = storage::safe_join(source_root, Path::new("next-book-id.json"))?;
    if let Some(counter) = read_optional(&counter_path)? {
        serde_json::from_slice::<u64>(&counter)
            .map_err(|e| format!("Contador de origen ilegible: {e}"))?;
        atomic_write(&destination.join("next-book-id.json"), &counter)?;
    }
    catalog.save(books)
}

pub fn default_folder() -> Result<PathBuf, String> {
    #[cfg(windows)]
    let base = std::env::var_os("LOCALAPPDATA").map(PathBuf::from);
    #[cfg(not(windows))]
    let base = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/share")));
    base.map(|p| p.join("MyCalibro"))
        .ok_or("No se pudo localizar la carpeta de datos del usuario".into())
}

pub fn startup_folder() -> Result<PathBuf, String> {
    let base = default_folder()?;
    match read_optional(&base.join("active-library.json"))? {
        Some(bytes) => serde_json::from_slice(&bytes)
            .map_err(|e| format!("Configuración de carpeta ilegible: {e}")),
        None => {
            fs::create_dir_all(&base).map_err(|e| e.to_string())?;
            Ok(base)
        }
    }
}

pub fn remember_folder(folder: &Path) -> Result<(), String> {
    let base = default_folder()?;
    fs::create_dir_all(&base).map_err(|e| e.to_string())?;
    let absolute = fs::canonicalize(folder).map_err(|e| e.to_string())?;
    atomic_write(
        &base.join("active-library.json"),
        &serde_json::to_vec(&absolute).map_err(|e| e.to_string())?,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    fn books() -> Vec<Book> {
        decode(br#"[{"id":1,"title":"Libro","author":"A","year":"","description":"","tags":[]}]"#)
            .unwrap()
    }

    #[test]
    fn recovery_preserves_managed_relative_files_covers_and_external_references() {
        let root = tempfile::tempdir().unwrap();
        let external = tempfile::tempdir().unwrap();
        let destination = tempfile::tempdir().unwrap();
        let source = external.path().join("original.txt");
        fs::write(&source, b"original text").unwrap();
        let mut original = books();
        original[0].files.push(
            crate::storage::import_file(
                &source,
                root.path(),
                1,
                "A",
                "Libro",
                crate::storage::StorageMode::Managed,
                crate::storage::Layout::Preserve,
                None,
            )
            .unwrap(),
        );
        original[0].file_path = Some(source.to_string_lossy().into_owned());
        original[0].bibliography.cover =
            Some(crate::book_metadata::preserve_cover(&source, root.path(), 1).unwrap());
        let snapshot = root.path().join("library.previous.json");
        fs::write(&snapshot, serde_json::to_vec(&original).unwrap()).unwrap();
        fs::write(
            root.path().join("library.json"),
            b"corrupt current catalogue",
        )
        .unwrap();
        fs::write(root.path().join("next-book-id.json"), b"50").unwrap();
        recover_into(&snapshot, destination.path()).unwrap();
        let (mut catalog, recovered) = Catalog::open(destination.path()).unwrap();
        assert_eq!(recovered[0].file_path, original[0].file_path);
        for file in recovered[0]
            .files
            .iter()
            .chain(recovered[0].bibliography.cover.iter())
        {
            assert_eq!(
                fs::read(file.resolve(destination.path()).unwrap()).unwrap(),
                b"original text"
            );
        }
        assert_eq!(catalog.allocate_id(&recovered).unwrap(), 50);
        assert_eq!(
            fs::read(root.path().join("library.json")).unwrap(),
            b"corrupt current catalogue"
        );
        assert_eq!(fs::read(source).unwrap(), b"original text");
    }

    #[test]
    fn recovery_never_publishes_catalogue_when_a_managed_file_is_missing_or_changed() {
        let root = tempfile::tempdir().unwrap();
        let destination = tempfile::tempdir().unwrap();
        let mut original = books();
        original[0].files.push(crate::storage::StoredFile {
            path: "books/missing.txt".into(),
            mode: crate::storage::StorageMode::Managed,
            sha256: "expected".into(),
        });
        assert!(recover_books_into(&original, root.path(), destination.path()).is_err());
        assert!(!destination.path().join("library.json").exists());
        let second = tempfile::tempdir().unwrap();
        fs::create_dir(root.path().join("books")).unwrap();
        fs::write(root.path().join("books/missing.txt"), b"changed").unwrap();
        assert!(recover_books_into(&original, root.path(), second.path()).is_err());
        assert!(!second.path().join("library.json").exists());
        assert_eq!(
            fs::read(root.path().join("books/missing.txt")).unwrap(),
            b"changed"
        );
    }

    #[test]
    fn reserved_book_ids_survive_deletion_restart_and_failed_imports() {
        let dir = tempfile::tempdir().unwrap();
        let (mut catalog, _) = Catalog::open(dir.path()).unwrap();
        assert_eq!(catalog.allocate_id(&[]).unwrap(), 1);
        assert_eq!(catalog.allocate_id(&[]).unwrap(), 2);
        catalog.save(&books()).unwrap();
        catalog.save(&[]).unwrap();
        drop(catalog);
        let (mut catalog, books) = Catalog::open(dir.path()).unwrap();
        assert!(books.is_empty());
        assert_eq!(catalog.allocate_id(&books).unwrap(), 3);
        fs::write(dir.path().join("next-book-id.json"), b"corrupt").unwrap();
        assert!(catalog.allocate_id(&books).is_err());
        assert_eq!(
            fs::read(dir.path().join("next-book-id.json")).unwrap(),
            b"corrupt"
        );
    }

    #[test]
    fn corrupt_catalog_is_preserved_and_never_loaded_as_examples() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("library.json");
        fs::write(&path, b"{truncated").unwrap();
        assert!(Catalog::open(dir.path()).is_err());
        assert_eq!(fs::read(path).unwrap(), b"{truncated");
    }

    #[test]
    fn saves_reopen_and_recover_previous_version_without_touching_source() {
        let dir = tempfile::tempdir().unwrap();
        let (mut catalog, initial) = Catalog::open(dir.path()).unwrap();
        assert!(initial.is_empty());
        let mut items = books();
        catalog.save(&items).unwrap();
        let original = fs::read(&catalog.path).unwrap();
        items[0].title = "Editado".into();
        catalog.save(&items).unwrap();
        let backup = dir.path().join("library.previous.json");
        assert_eq!(fs::read(&backup).unwrap(), original);
        drop(catalog);
        let (_, reopened) = Catalog::open(dir.path()).unwrap();
        assert_eq!(reopened[0].title, "Editado");
        let recovered = tempfile::tempdir().unwrap();
        recover_into(&backup, recovered.path()).unwrap();
        assert_eq!(Catalog::open(recovered.path()).unwrap().1[0].title, "Libro");
        assert!(recover_into(&backup, dir.path()).is_err());
        assert_eq!(fs::read(backup).unwrap(), original);
    }

    #[test]
    fn concurrent_instances_and_external_changes_cannot_overwrite_catalog() {
        let dir = tempfile::tempdir().unwrap();
        let (mut catalog, _) = Catalog::open(dir.path()).unwrap();
        assert!(Catalog::open(dir.path()).is_err());
        catalog.save(&books()).unwrap();
        fs::write(&catalog.path, b"external version").unwrap();
        assert!(catalog.save(&[]).is_err());
        assert_eq!(fs::read(&catalog.path).unwrap(), b"external version");
    }

    #[test]
    fn missing_catalog_with_backup_and_unavailable_folder_fail_closed() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("library.previous.json"), b"[]").unwrap();
        assert!(Catalog::open(dir.path()).is_err());
        assert!(Catalog::open(&dir.path().join("missing")).is_err());
        assert!(!dir.path().join("missing").exists());
    }

    #[test]
    fn interrupted_write_leaves_current_catalog_and_failed_backup_blocks_save() {
        let dir = tempfile::tempdir().unwrap();
        let (mut catalog, _) = Catalog::open(dir.path()).unwrap();
        catalog.save(&books()).unwrap();
        let before = fs::read(&catalog.path).unwrap();
        // An interrupted staging write never becomes the committed catalog.
        let mut staged = tempfile::NamedTempFile::new_in(dir.path()).unwrap();
        staged.write_all(b"partial").unwrap();
        drop(staged);
        fs::create_dir(dir.path().join("library.previous.json")).unwrap();
        assert!(catalog.save(&[]).is_err());
        assert_eq!(fs::read(&catalog.path).unwrap(), before);
    }

    #[test]
    fn duplicate_ids_and_invalid_recovery_are_rejected() {
        let mut items = books();
        items.push(items[0].clone());
        assert!(decode(&serde_json::to_vec(&items).unwrap()).is_err());
        let dir = tempfile::tempdir().unwrap();
        let dest = tempfile::tempdir().unwrap();
        let source = dir.path().join("bad.json");
        fs::write(&source, b"bad").unwrap();
        assert!(recover_into(&source, dest.path()).is_err());
        assert_eq!(fs::read_dir(dest.path()).unwrap().count(), 0);
    }
}
