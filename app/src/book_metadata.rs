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
