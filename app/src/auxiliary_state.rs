//! Optimistic concurrency and recovery for small library state files.
use serde::{de::DeserializeOwned, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
};

#[derive(Default)]
pub struct StateFiles {
    expected: BTreeMap<String, Option<Vec<u8>>>,
    corrupt: BTreeSet<String>,
    pub error: Option<String>,
}
impl StateFiles {
    pub fn load<T: DeserializeOwned + Default>(&mut self, root: &Path, name: &str) -> T {
        let result = crate::storage::safe_join(root, Path::new(name)).and_then(|path| {
            match std::fs::read(path) {
                Ok(bytes) => Ok(Some(bytes)),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
                Err(e) => Err(e.to_string()),
            }
        });
        match result {
            Ok(bytes) => {
                self.expected.insert(name.into(), bytes.clone());
                match bytes.map(|b| serde_json::from_slice(&b)).transpose() {
                    Ok(value) => value.unwrap_or_default(),
                    Err(e) => {
                        self.corrupt.insert(name.into());
                        self.error = Some(format!(
                            "{name} ilegible; se conservará antes de guardar: {e}"
                        ));
                        T::default()
                    }
                }
            }
            Err(e) => {
                self.error = Some(format!("No se pudo leer {name}: {e}"));
                T::default()
            }
        }
    }
    pub fn save<T: Serialize>(&mut self, root: &Path, name: &str, value: &T) -> Result<(), String> {
        let expected = self
            .expected
            .get(name)
            .ok_or("Estado no leído; guardado bloqueado")?;
        let path = crate::storage::safe_join(root, Path::new(name))?;
        let current = match std::fs::read(&path) {
            Ok(bytes) => Some(bytes),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
            Err(e) => return Err(e.to_string()),
        };
        if &current != expected {
            return Err(format!(
                "{name} cambió fuera de MyCalibro; se conserva el archivo externo"
            ));
        }
        let bytes = serde_json::to_vec_pretty(value).map_err(|e| e.to_string())?;
        if current.as_ref() == Some(&bytes) {
            return Ok(());
        }
        if self.corrupt.contains(name) {
            use std::io::Write;
            let mut recovery = tempfile::Builder::new()
                .prefix(&format!("{name}.recovery-"))
                .tempfile_in(root)
                .map_err(|e| e.to_string())?;
            recovery
                .write_all(current.as_deref().unwrap_or_default())
                .map_err(|e| e.to_string())?;
            recovery.as_file().sync_all().map_err(|e| e.to_string())?;
            recovery.keep().map_err(|e| e.to_string())?;
            self.corrupt.remove(name);
        }
        crate::persistence::atomic_write(&path, &bytes)?;
        self.expected.insert(name.into(), Some(bytes));
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn corrupt_state_is_preserved_and_external_edits_block_writes() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("positions.json");
        std::fs::write(&path, b"broken").unwrap();
        let mut state = StateFiles::default();
        let v: Vec<u32> = state.load(root.path(), "positions.json");
        assert!(v.is_empty());
        state.save(root.path(), "positions.json", &vec![2]).unwrap();
        assert!(std::fs::read_dir(root.path())
            .unwrap()
            .flatten()
            .any(|p| std::fs::read(p.path()).unwrap() == b"broken"));
        std::fs::write(&path, b"[42]").unwrap();
        assert!(state.save(root.path(), "positions.json", &vec![3]).is_err());
        assert_eq!(std::fs::read(path).unwrap(), b"[42]");
    }
}
