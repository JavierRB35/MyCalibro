//! Execute a reviewed batch outside the UI thread. Catalog writes stay with its owner.
use crate::storage::{self, Preferences, StoredFile};
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::atomic::{AtomicBool, Ordering},
};

#[derive(Clone, Debug, Default)]
pub struct Metadata {
    pub title: String,
    pub author: String,
    pub year: String,
    pub description: String,
    pub tags: Vec<String>,
    pub bibliography: crate::book_metadata::Bibliography,
    pub cover_source: Option<PathBuf>,
}

impl Metadata {
    pub fn from_calibre(record: &crate::calibre_import::CalibreRecord) -> Self {
        Self {
            title: record.title.clone(),
            author: record.author.clone(),
            year: record.year.clone(),
            description: record.description.clone(),
            tags: record.tags.clone(),
            bibliography: crate::book_metadata::Bibliography {
                source: format!(
                    "Calibre, registro {} (importación puntual)",
                    record.calibre_id
                ),
                identifiers: record.identifiers.clone(),
                cover: None,
            },
            cover_source: record.cover.clone(),
        }
    }
    fn from_fields(fields: HashMap<String, String>) -> Self {
        let has_fields = !fields.is_empty();
        Self {
            bibliography: crate::book_metadata::Bibliography {
                source: if has_fields {
                    "Metadatos incrustados (ebook-meta)"
                } else {
                    "Nombre del archivo; metadatos no disponibles"
                }
                .into(),
                ..Default::default()
            },
            cover_source: None,
            title: fields.get("title").cloned().unwrap_or_default(),
            author: fields.get("author").cloned().unwrap_or_default(),
            year: fields.get("year").cloned().unwrap_or_default(),
            description: fields.get("description").cloned().unwrap_or_default(),
            tags: fields
                .get("tags")
                .map(|s| {
                    s.split(',')
                        .map(str::trim)
                        .filter(|s| !s.is_empty())
                        .map(str::to_owned)
                        .collect()
                })
                .unwrap_or_default(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct ImportRequest {
    /// A durably reserved new ID, or an existing ID explicitly chosen for association.
    pub book_id: u64,
    pub associate: bool,
    pub path: PathBuf,
    pub source_base: Option<PathBuf>,
    /// Original Calibre metadata, or target-book metadata for an association.
    pub metadata: Option<Metadata>,
}

pub struct PreparedImport {
    pub book_id: u64,
    pub associate: bool,
    pub metadata: Metadata,
    pub file: StoredFile,
    pub warning: Option<String>,
}

pub struct ImportOutcome {
    pub source: PathBuf,
    pub result: Result<PreparedImport, String>,
}

pub fn prepare(
    request: &ImportRequest,
    root: &Path,
    preferences: &Preferences,
    read_metadata: &impl Fn(&Path) -> Result<HashMap<String, String>, String>,
) -> Result<PreparedImport, String> {
    if !request.path.is_file() {
        return Err("No se encontró el archivo de origen".into());
    }
    let (mut metadata, mut warning) = match request.metadata.clone() {
        Some(metadata) => (metadata, None),
        None => match read_metadata(&request.path) {
            Ok(fields) => (Metadata::from_fields(fields), None),
            Err(error) => (
                Metadata::default(),
                Some(format!("Metadatos incrustados no disponibles: {error}")),
            ),
        },
    };
    if metadata.title.trim().is_empty() {
        metadata.title = request
            .path
            .file_stem()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned();
    }
    let file = storage::import_file(
        &request.path,
        root,
        request.book_id,
        &metadata.author,
        &metadata.title,
        preferences.import_mode,
        preferences.layout,
        request.source_base.as_deref(),
    )?;
    if let Some(source) = &metadata.cover_source {
        match crate::book_metadata::preserve_cover(source, root, request.book_id) {
            Ok(cover) => metadata.bibliography.cover = Some(cover),
            Err(error) => warning = Some(format!("Portada no copiada: {error}")),
        }
    }
    Ok(PreparedImport {
        book_id: request.book_id,
        associate: request.associate,
        metadata,
        file,
        warning,
    })
}

/// Call in a worker. Cancellation is checked between files; completed results are retained.
/// A metadata failure yields an explicit warning; a missing/copy-failed file yields an error.
pub fn run_batch(
    requests: &[ImportRequest],
    root: &Path,
    preferences: &Preferences,
    cancel: &AtomicBool,
    read_metadata: impl Fn(&Path) -> Result<HashMap<String, String>, String>,
    mut on_progress: impl FnMut(usize, usize),
) -> Vec<ImportOutcome> {
    let mut outcomes = Vec::new();
    on_progress(0, requests.len());
    for request in requests {
        if cancel.load(Ordering::Relaxed) {
            break;
        }
        let result = prepare(request, root, preferences, &read_metadata);
        outcomes.push(ImportOutcome {
            source: request.path.clone(),
            result,
        });
        on_progress(outcomes.len(), requests.len());
    }
    outcomes
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    fn request(path: PathBuf, id: u64) -> ImportRequest {
        ImportRequest {
            book_id: id,
            associate: false,
            path,
            source_base: None,
            metadata: None,
        }
    }
    #[test]
    fn managed_batch_preserves_sources_reports_partial_errors_and_never_writes_catalog() {
        let root = tempfile::tempdir().unwrap();
        let origin = tempfile::tempdir().unwrap();
        let source = origin.path().join("book.txt");
        fs::write(&source, b"original").unwrap();
        let requests = vec![
            request(origin.path().join("missing.txt"), 1),
            request(source.clone(), 2),
        ];
        let preferences = Preferences {
            import_mode: storage::StorageMode::Managed,
            ..Default::default()
        };
        let mut progress = Vec::new();
        let results = run_batch(
            &requests,
            root.path(),
            &preferences,
            &AtomicBool::new(false),
            |_| Err("Calibre no instalado".into()),
            |done, total| progress.push((done, total)),
        );
        assert!(results[0].result.is_err());
        let prepared = results[1].result.as_ref().unwrap();
        assert_eq!(results[1].source, source);
        assert_eq!(prepared.metadata.title, "book");
        assert!(prepared.warning.is_some());
        assert_eq!(prepared.file.mode, storage::StorageMode::Managed);
        assert_eq!(
            fs::read(prepared.file.resolve(root.path()).unwrap()).unwrap(),
            b"original"
        );
        assert_eq!(fs::read(source).unwrap(), b"original");
        assert!(!root.path().join("library.json").exists());
        assert_eq!(progress, vec![(0, 2), (1, 2), (2, 2)]);
    }
    #[test]
    fn supplied_calibre_metadata_and_association_target_are_preserved() {
        let root = tempfile::tempdir().unwrap();
        let origin = tempfile::tempdir().unwrap();
        let source = origin.path().join("different-name.txt");
        fs::write(&source, b"original").unwrap();
        let request = ImportRequest {
            associate: true,
            metadata: Some(Metadata {
                title: "Original title".into(),
                author: "Original author".into(),
                year: "2020".into(),
                description: "Original description".into(),
                tags: vec!["Original tag".into()],
                ..Default::default()
            }),
            ..request(source.clone(), 42)
        };
        let prepared = prepare(&request, root.path(), &Preferences::default(), &|_| {
            panic!("must not replace supplied metadata")
        })
        .unwrap();
        assert_eq!(prepared.book_id, 42);
        assert!(prepared.associate);
        assert!(prepared.warning.is_none());
        assert_eq!(prepared.metadata.title, "Original title");
        assert_eq!(prepared.metadata.author, "Original author");
        assert_eq!(prepared.metadata.year, "2020");
        assert_eq!(prepared.metadata.description, "Original description");
        assert_eq!(prepared.metadata.tags, vec!["Original tag"]);
        assert_eq!(
            prepared.file.resolve(root.path()).unwrap(),
            fs::canonicalize(source).unwrap()
        );
    }
    #[test]
    fn cancellation_retains_completed_results_and_does_not_process_remaining_files() {
        let root = tempfile::tempdir().unwrap();
        let origin = tempfile::tempdir().unwrap();
        let source = origin.path().join("book.txt");
        fs::write(&source, b"original").unwrap();
        let requests = vec![request(source.clone(), 1), request(source, 2)];
        let cancel = AtomicBool::new(false);
        let outcomes = run_batch(
            &requests,
            root.path(),
            &Preferences::default(),
            &cancel,
            |_| Ok(HashMap::new()),
            |done, _| {
                if done == 1 {
                    cancel.store(true, Ordering::Relaxed);
                }
            },
        );
        assert_eq!(outcomes.len(), 1);
        assert!(outcomes[0].result.is_ok());
    }
}
