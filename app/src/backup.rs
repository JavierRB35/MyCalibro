//! Complete snapshots with SHA-256 manifests; restore always creates a new library.
use crate::storage::{self, StorageMode, StoredFile};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{
    collections::HashSet,
    fs,
    path::{Path, PathBuf},
};

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Entry {
    path: PathBuf,
    size: u64,
    sha256: String,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct External {
    book_id: u64,
    // None denotes the legacy file_path field.
    file_index: Option<usize>,
    payload: PathBuf,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Manifest {
    version: u32,
    entries: Vec<Entry>,
    external: Vec<External>,
    exclusions: Vec<String>,
}

pub struct BackupReport {
    pub path: PathBuf,
    pub exclusions: Vec<String>,
}

fn collect(root: &Path, relative: &Path, files: &mut Vec<PathBuf>) -> Result<(), String> {
    let folder = if relative.as_os_str().is_empty() {
        root.to_owned()
    } else {
        storage::safe_join(root, relative)?
    };
    for item in fs::read_dir(folder).map_err(|e| e.to_string())? {
        let item = item.map_err(|e| e.to_string())?;
        let path = relative.join(item.file_name());
        if relative.as_os_str().is_empty()
            && [".mycalibro.lock", "library.json", "active-library.json"]
                .iter()
                .any(|name| item.file_name() == *name)
        {
            continue;
        }
        let meta = fs::symlink_metadata(item.path()).map_err(|e| e.to_string())?;
        if meta.file_type().is_symlink() {
            return Err(format!(
                "No se sigue el enlace {} durante el respaldo",
                item.path().display()
            ));
        }
        storage::safe_join(root, &path)?;
        if meta.is_dir() {
            collect(root, &path, files)?;
        } else if meta.is_file() {
            files.push(path);
        } else {
            return Err(format!(
                "Tipo de archivo no respaldable: {}",
                item.path().display()
            ));
        }
    }
    Ok(())
}

fn add_file(
    source: &Path,
    stage: &Path,
    relative: &Path,
    manifest: &mut Manifest,
) -> Result<(), String> {
    let target = storage::safe_join(stage, relative)?;
    fs::create_dir_all(target.parent().ok_or("Destino sin carpeta")?).map_err(|e| e.to_string())?;
    let sha256 = storage::copy_new(source, &target)?;
    let size = fs::metadata(&target).map_err(|e| e.to_string())?.len();
    manifest.entries.push(Entry {
        path: relative.to_owned(),
        size,
        sha256,
    });
    Ok(())
}

fn add_external(
    source: &Path,
    id: u64,
    index: Option<usize>,
    include: bool,
    stage: &Path,
    manifest: &mut Manifest,
) -> Result<(), String> {
    if !include || !source.is_file() {
        manifest.exclusions.push(format!(
            "Libro {id}, {}: {}",
            source.display(),
            if include {
                "archivo externo inaccesible"
            } else {
                "archivo externo excluido por configuración"
            }
        ));
        return Ok(());
    }
    let suffix = index
        .map(|n| n.to_string())
        .unwrap_or_else(|| "legacy".into());
    let payload = PathBuf::from("external")
        .join(id.to_string())
        .join(suffix)
        .join(source.file_name().ok_or("Archivo sin nombre")?);
    add_file(source, stage, &payload, manifest)?;
    manifest.external.push(External {
        book_id: id,
        file_index: index,
        payload,
    });
    Ok(())
}

pub fn create(
    root: &Path,
    catalog: &[u8],
    destination_parent: &Path,
    include_external: bool,
) -> Result<BackupReport, String> {
    crate::persistence::decode(catalog)?;
    let root = fs::canonicalize(root).map_err(|e| e.to_string())?;
    let parent = fs::canonicalize(destination_parent).map_err(|e| e.to_string())?;
    if parent.starts_with(&root) {
        return Err("El respaldo debe guardarse fuera de la biblioteca".into());
    }
    let stage = tempfile::Builder::new()
        .prefix("MyCalibro-backup-")
        .tempdir_in(parent)
        .map_err(|e| e.to_string())?;
    let mut manifest = Manifest {
        version: 1,
        entries: vec![],
        external: vec![],
        exclusions: vec![],
    };
    let mut files = Vec::new();
    collect(&root, Path::new(""), &mut files)?;
    files.sort();
    for relative in files {
        add_file(
            &storage::safe_join(&root, &relative)?,
            stage.path(),
            &PathBuf::from("library").join(relative),
            &mut manifest,
        )?;
    }
    let mut values: Vec<Value> = serde_json::from_slice(catalog).map_err(|e| e.to_string())?;
    for book in &mut values {
        let id = book["id"].as_u64().ok_or("ID de libro inválido")?;
        if let Some(path) = book["file_path"].as_str() {
            add_external(
                Path::new(path),
                id,
                None,
                include_external,
                stage.path(),
                &mut manifest,
            )?;
        }
        if let Some(files) = book.get_mut("files").and_then(Value::as_array_mut) {
            for (index, value) in files.iter_mut().enumerate() {
                let file: StoredFile =
                    serde_json::from_value(value.clone()).map_err(|e| e.to_string())?;
                let source = file.resolve(&root)?;
                if file.mode == StorageMode::Linked {
                    add_external(
                        &source,
                        id,
                        Some(index),
                        include_external,
                        stage.path(),
                        &mut manifest,
                    )?;
                    // External files may have changed since import: the snapshot records
                    // the hash of the bytes actually backed up, without editing the live catalog.
                    if let Some(reference) = manifest
                        .external
                        .iter()
                        .find(|e| e.book_id == id && e.file_index == Some(index))
                    {
                        let entry = manifest
                            .entries
                            .iter()
                            .find(|e| e.path == reference.payload)
                            .ok_or("Falta la copia externa")?;
                        value["sha256"] = Value::String(entry.sha256.clone());
                    }
                } else {
                    let payload = PathBuf::from("library").join(&file.path);
                    let entry = manifest
                        .entries
                        .iter()
                        .find(|e| e.path == payload)
                        .ok_or_else(|| {
                            format!("Falta el archivo administrado {}", file.path.display())
                        })?;
                    if !file.sha256.is_empty() && entry.sha256 != file.sha256 {
                        return Err(format!(
                            "Ha cambiado el archivo administrado {}",
                            file.path.display()
                        ));
                    }
                }
            }
        }
    }
    let catalog_path = stage.path().join("catalog.json");
    let catalog = serde_json::to_vec_pretty(&values).map_err(|e| e.to_string())?;
    crate::persistence::atomic_write(&catalog_path, &catalog)?;
    manifest.entries.push(Entry {
        path: PathBuf::from("catalog.json"),
        size: catalog.len() as u64,
        sha256: storage::hash_file(&catalog_path)?,
    });
    crate::persistence::atomic_write(
        &stage.path().join("manifest.json"),
        &serde_json::to_vec_pretty(&manifest).map_err(|e| e.to_string())?,
    )?;
    verify(stage.path())?;
    Ok(BackupReport {
        path: stage.keep(),
        exclusions: manifest.exclusions,
    })
}

fn read_verified(backup: &Path) -> Result<Manifest, String> {
    let bytes = fs::read(storage::safe_join(backup, Path::new("manifest.json"))?)
        .map_err(|e| e.to_string())?;
    let manifest: Manifest =
        serde_json::from_slice(&bytes).map_err(|e| format!("Manifiesto ilegible: {e}"))?;
    if manifest.version != 1 {
        return Err("Versión de respaldo no admitida".into());
    }
    let mut seen = HashSet::new();
    for entry in &manifest.entries {
        if !seen.insert(entry.path.to_string_lossy().to_lowercase()) {
            return Err("El manifiesto contiene rutas duplicadas".into());
        }
        if entry.path != Path::new("catalog.json")
            && !entry.path.starts_with("library")
            && !entry.path.starts_with("external")
        {
            return Err("Contenido inesperado en el manifiesto".into());
        }
        let path = storage::safe_join(backup, &entry.path)?;
        if fs::metadata(&path).map_err(|e| e.to_string())?.len() != entry.size
            || storage::hash_file(&path)? != entry.sha256
        {
            return Err(format!("Integridad incorrecta: {}", entry.path.display()));
        }
    }
    if !seen.contains("catalog.json") {
        return Err("Falta el catálogo en el manifiesto".into());
    }
    let books = crate::persistence::decode(&catalog_snapshot(backup, &manifest)?)?;
    for book in &books {
        for file in &book.files {
            if file.mode == StorageMode::Managed {
                let relative = PathBuf::from("library").join(&file.path);
                storage::safe_join(backup, &relative)?;
                let entry = manifest
                    .entries
                    .iter()
                    .find(|e| e.path == relative)
                    .ok_or("Falta un archivo administrado en el manifiesto")?;
                if !file.sha256.is_empty() && entry.sha256 != file.sha256 {
                    return Err("El manifiesto no coincide con la huella del libro".into());
                }
            }
        }
    }
    let mut external_refs = HashSet::new();
    for external in &manifest.external {
        if !external_refs.insert((external.book_id, external.file_index)) {
            return Err("Referencia externa duplicada".into());
        }
        let book = books
            .iter()
            .find(|b| b.id == external.book_id)
            .ok_or("Referencia externa a libro inexistente")?;
        match external.file_index {
            Some(index)
                if book
                    .files
                    .get(index)
                    .is_some_and(|f| f.mode == StorageMode::Linked) =>
            {
                ()
            }
            None if book.file_path.is_some() => (),
            _ => return Err("Referencia externa inválida".into()),
        }
        if !external.payload.starts_with("external")
            || !manifest.entries.iter().any(|e| e.path == external.payload)
        {
            return Err("Referencia externa no respaldada".into());
        }
    }
    Ok(manifest)
}

pub fn verify(backup: &Path) -> Result<Vec<String>, String> {
    Ok(read_verified(backup)?.exclusions)
}

fn catalog_snapshot(backup: &Path, manifest: &Manifest) -> Result<Vec<u8>, String> {
    let bytes = fs::read(storage::safe_join(backup, Path::new("catalog.json"))?)
        .map_err(|e| e.to_string())?;
    let expected = manifest
        .entries
        .iter()
        .find(|e| e.path == Path::new("catalog.json"))
        .ok_or("Falta el catálogo")?;
    if format!("{:x}", Sha256::digest(&bytes)) != expected.sha256 {
        return Err("El catálogo del respaldo cambió durante la restauración".into());
    }
    Ok(bytes)
}

pub fn restore(backup: &Path, destination_parent: &Path) -> Result<BackupReport, String> {
    let manifest = read_verified(backup)?;
    let parent = fs::canonicalize(destination_parent).map_err(|e| e.to_string())?;
    if parent.starts_with(fs::canonicalize(backup).map_err(|e| e.to_string())?) {
        return Err("Elija un destino fuera del respaldo".into());
    }
    let stage = tempfile::Builder::new()
        .prefix("MyCalibro-restored-")
        .tempdir_in(parent)
        .map_err(|e| e.to_string())?;
    for entry in &manifest.entries {
        if entry.path == Path::new("catalog.json") {
            continue;
        }
        let relative = entry.path.strip_prefix("library").unwrap_or(&entry.path);
        // Catalog is supplied exclusively by catalog.json, never by another manifest entry.
        if relative == Path::new("library.json")
            || relative == Path::new(".mycalibro.lock")
            || relative == Path::new("active-library.json")
        {
            return Err("Entrada reservada en el manifiesto".into());
        }
        let target = storage::safe_join(stage.path(), relative)?;
        fs::create_dir_all(target.parent().ok_or("Destino sin carpeta")?)
            .map_err(|e| e.to_string())?;
        let hash = storage::copy_new(&storage::safe_join(backup, &entry.path)?, &target)?;
        if hash != entry.sha256 {
            return Err("El respaldo cambió durante la restauración".into());
        }
    }
    let mut catalog: Vec<Value> =
        serde_json::from_slice(&catalog_snapshot(backup, &manifest)?).map_err(|e| e.to_string())?;
    for external in &manifest.external {
        let book = catalog
            .iter_mut()
            .find(|b| b["id"].as_u64() == Some(external.book_id))
            .ok_or("Referencia externa a libro inexistente")?;
        let path = storage::safe_join(stage.path(), &external.payload)?;
        match external.file_index {
            Some(index) => {
                let file = book
                    .get_mut("files")
                    .and_then(Value::as_array_mut)
                    .and_then(|f| f.get_mut(index))
                    .ok_or("Índice de formato inválido")?;
                let parsed: StoredFile =
                    serde_json::from_value(file.clone()).map_err(|e| e.to_string())?;
                if parsed.mode != StorageMode::Linked {
                    return Err("Se intentó sustituir una ruta administrada como externa".into());
                }
                file["path"] = serde_json::to_value(path).map_err(|e| e.to_string())?;
            }
            None => book["file_path"] = serde_json::to_value(path).map_err(|e| e.to_string())?,
        }
    }
    let bytes = serde_json::to_vec_pretty(&catalog).map_err(|e| e.to_string())?;
    let books = crate::persistence::decode(&bytes)?;
    // Verify managed references as well as the manifest, including missing payload entries.
    for book in &catalog {
        if let Some(files) = book["files"].as_array() {
            for file in files {
                let file: StoredFile =
                    serde_json::from_value(file.clone()).map_err(|e| e.to_string())?;
                if file.mode == StorageMode::Managed {
                    let actual = storage::hash_file(&file.resolve(stage.path())?)?;
                    if !file.sha256.is_empty() && actual != file.sha256 {
                        return Err("Archivo administrado corrupto tras restaurar".into());
                    }
                }
            }
        }
    }
    let (mut destination, _) = crate::persistence::Catalog::open(stage.path())?;
    destination.save(&books)?;
    drop(destination);
    Ok(BackupReport {
        path: stage.keep(),
        exclusions: manifest.exclusions,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn managed_and_linked_formats_restore_with_integrity_and_modes() {
        let root = tempfile::tempdir().unwrap();
        let output = tempfile::tempdir().unwrap();
        let source_dir = tempfile::tempdir().unwrap();
        let source = source_dir.path().join("book.txt");
        fs::write(&source, b"managed and linked").unwrap();
        let managed = storage::import_file(
            &source,
            root.path(),
            1,
            "A",
            "T",
            StorageMode::Managed,
            storage::Layout::Preserve,
            None,
        )
        .unwrap();
        let linked = storage::import_file(
            &source,
            root.path(),
            1,
            "A",
            "T",
            StorageMode::Linked,
            storage::Layout::Preserve,
            None,
        )
        .unwrap();
        let mut catalog: Value = serde_json::from_slice(&legacy_catalog(&source)).unwrap();
        catalog[0]["file_path"] = Value::Null;
        catalog[0]["files"] = serde_json::to_value(vec![managed.clone(), linked]).unwrap();
        let catalog = serde_json::to_vec(&catalog).unwrap();
        // A linked source can change independently; recover the bytes actually captured.
        fs::write(&source, b"linked updated externally").unwrap();
        let snapshot = create(root.path(), &catalog, output.path(), true).unwrap();
        let restored = restore(&snapshot.path, output.path()).unwrap();
        let (_, books) = crate::persistence::Catalog::open(&restored.path).unwrap();
        assert_eq!(books[0].files[0].mode, StorageMode::Managed);
        assert_eq!(books[0].files[1].mode, StorageMode::Linked);
        for file in &books[0].files {
            assert_eq!(
                storage::hash_file(&file.resolve(&restored.path).unwrap()).unwrap(),
                file.sha256
            );
        }
        fs::remove_file(managed.resolve(root.path()).unwrap()).unwrap();
        assert!(create(root.path(), &catalog, output.path(), true).is_err());
    }
    fn legacy_catalog(path: &Path) -> Vec<u8> {
        serde_json::to_vec(&serde_json::json!([{"id":1,"title":"Libro","author":"A","year":"","description":"","tags":[],"file_path":path}])).unwrap()
    }
    #[test]
    fn full_backup_restores_catalog_preferences_and_external_bytes_without_touching_origin() {
        let root = tempfile::tempdir().unwrap();
        let output = tempfile::tempdir().unwrap();
        let origin = tempfile::tempdir().unwrap();
        let source = origin.path().join("book.txt");
        fs::write(&source, b"original").unwrap();
        fs::write(root.path().join("reader.json"), b"{\"position\":4}").unwrap();
        let snapshot = create(root.path(), &legacy_catalog(&source), output.path(), true).unwrap();
        assert!(snapshot.exclusions.is_empty());
        let restored = restore(&snapshot.path, output.path()).unwrap();
        let (_, books) = crate::persistence::Catalog::open(&restored.path).unwrap();
        let recovered = PathBuf::from(books[0].file_path.as_ref().unwrap());
        assert_ne!(recovered, source);
        assert_eq!(fs::read(recovered).unwrap(), b"original");
        assert_eq!(fs::read(source).unwrap(), b"original");
        assert_eq!(
            fs::read(restored.path.join("reader.json")).unwrap(),
            b"{\"position\":4}"
        );
        assert!(!root.path().join("library.json").exists());
    }
    #[test]
    fn excluded_and_missing_external_books_are_explicit_and_references_remain() {
        let root = tempfile::tempdir().unwrap();
        let output = tempfile::tempdir().unwrap();
        let source = root.path().join("missing.txt");
        for include in [true, false] {
            let snapshot = create(
                root.path(),
                &legacy_catalog(&source),
                output.path(),
                include,
            )
            .unwrap();
            assert_eq!(snapshot.exclusions.len(), 1);
            let restored = restore(&snapshot.path, output.path()).unwrap();
            assert_eq!(restored.exclusions.len(), 1);
            let (_, books) = crate::persistence::Catalog::open(&restored.path).unwrap();
            assert_eq!(Path::new(books[0].file_path.as_ref().unwrap()), source);
        }
    }
    #[test]
    fn corrupt_payload_or_traversal_cannot_restore_or_overwrite_existing_data() {
        let root = tempfile::tempdir().unwrap();
        let output = tempfile::tempdir().unwrap();
        fs::write(root.path().join("settings.json"), b"settings").unwrap();
        let snapshot = create(root.path(), b"[]", output.path(), false).unwrap();
        fs::write(snapshot.path.join("library/settings.json"), b"corrupt").unwrap();
        assert!(restore(&snapshot.path, output.path()).is_err());
        assert_eq!(
            fs::read(root.path().join("settings.json")).unwrap(),
            b"settings"
        );
        let mut manifest: Value =
            serde_json::from_slice(&fs::read(snapshot.path.join("manifest.json")).unwrap())
                .unwrap();
        manifest["entries"][0]["path"] = Value::String("../escape".into());
        fs::write(
            snapshot.path.join("manifest.json"),
            serde_json::to_vec(&manifest).unwrap(),
        )
        .unwrap();
        assert!(restore(&snapshot.path, output.path()).is_err());
    }
    #[test]
    fn refuses_recursive_destination_and_missing_manifest() {
        let root = tempfile::tempdir().unwrap();
        let dest = tempfile::tempdir().unwrap();
        assert!(create(root.path(), b"[]", root.path(), false).is_err());
        assert!(restore(root.path(), dest.path()).is_err());
        assert_eq!(fs::read_dir(dest.path()).unwrap().count(), 0);
    }
}
