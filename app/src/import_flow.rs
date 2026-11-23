//! UI integration: workers prepare files; only the app owner mutates/saves the catalogue.
use crate::{import_jobs, import_scan, import_wizard, import_wizard_view, Book, LibraryApp};
use eframe::egui;
use std::{
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc, Arc,
    },
};

enum Event {
    Progress(usize, usize),
    Done(Vec<import_jobs::ImportOutcome>),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        persistence::Catalog,
        storage::{Preferences, StorageMode},
    };
    use std::{collections::HashMap, fs};

    #[test]
    fn reviewed_import_and_association_survive_reopen_without_changing_source_or_metadata() {
        let root = tempfile::tempdir().unwrap();
        let source = tempfile::tempdir().unwrap();
        let first = source.path().join("original.txt");
        let second = source.path().join("edition.pdf");
        fs::write(&first, b"original text").unwrap();
        fs::write(&second, b"%PDF-original").unwrap();
        let (mut catalog, mut books) = Catalog::open(root.path()).unwrap();
        let id = catalog.allocate_id(&books).unwrap();
        let preferences = Preferences {
            import_mode: StorageMode::Managed,
            ..Default::default()
        };
        let mut request = import_jobs::ImportRequest {
            book_id: id,
            associate: false,
            path: first.clone(),
            source_base: Some(source.path().into()),
            metadata: None,
        };
        let prepared = import_jobs::prepare(&request, root.path(), &preferences, &|_| {
            Ok(HashMap::from([("title".into(), "Original title".into())]))
        })
        .unwrap();
        apply_prepared(&mut books, prepared).unwrap();
        catalog.save(&books).unwrap();
        request.associate = true;
        request.path = second.clone();
        let prepared = import_jobs::prepare(&request, root.path(), &preferences, &|_| {
            Ok(HashMap::from([("title".into(), "Different title".into())]))
        })
        .unwrap();
        apply_prepared(&mut books, prepared).unwrap();
        catalog.save(&books).unwrap();
        drop(catalog);
        let (_, reopened) = Catalog::open(root.path()).unwrap();
        assert_eq!(reopened.len(), 1);
        assert_eq!(reopened[0].title, "Original title");
        assert_eq!(reopened[0].files.len(), 2);
        assert_eq!(fs::read(first).unwrap(), b"original text");
        assert_eq!(fs::read(second).unwrap(), b"%PDF-original");
    }

    #[test]
    fn stale_association_and_reused_ids_never_replace_a_book() {
        let root = tempfile::tempdir().unwrap();
        let source = root.path().join("source.txt");
        fs::write(&source, b"text").unwrap();
        let mut request = import_jobs::ImportRequest {
            book_id: 42,
            associate: true,
            path: source,
            source_base: None,
            metadata: None,
        };
        let prepare = |r: &import_jobs::ImportRequest| {
            import_jobs::prepare(r, root.path(), &Preferences::default(), &|_| {
                Ok(HashMap::new())
            })
            .unwrap()
        };
        let mut books = Vec::new();
        assert!(apply_prepared(&mut books, prepare(&request)).is_err());
        request.associate = false;
        apply_prepared(&mut books, prepare(&request)).unwrap();
        let before = serde_json::to_vec(&books).unwrap();
        assert!(apply_prepared(&mut books, prepare(&request)).is_err());
        assert_eq!(serde_json::to_vec(&books).unwrap(), before);
    }
}

#[derive(Default)]
pub struct ImportFlow {
    wizard: Option<import_wizard_view::ImportWizardView>,
    receiver: Option<mpsc::Receiver<Event>>,
    cancel: Arc<AtomicBool>,
    generation: u64,
    progress: (usize, usize),
    pub results: Vec<String>,
    results_open: bool,
}

impl ImportFlow {
    pub fn busy(&self) -> bool {
        self.receiver.is_some()
    }
    pub fn active(&self) -> bool {
        self.busy() || self.wizard.is_some()
    }
}

fn metadata(book: &Book) -> import_jobs::Metadata {
    import_jobs::Metadata {
        title: book.title.clone(),
        author: book.author.clone(),
        year: book.year.clone(),
        description: book.description.clone(),
        tags: book.tags.clone(),
    }
}

/// Apply only successful, explicitly reviewed results. Association never replaces metadata.
fn apply_prepared(
    books: &mut Vec<Book>,
    prepared: import_jobs::PreparedImport,
) -> Result<u64, String> {
    let id = prepared.book_id;
    if prepared.associate {
        let book = books
            .iter_mut()
            .find(|b| b.id == id)
            .ok_or("La ficha destino ya no existe")?;
        if !book
            .files
            .iter()
            .any(|f| f.path == prepared.file.path && f.mode == prepared.file.mode)
        {
            book.files.push(prepared.file);
        }
    } else {
        if books.iter().any(|b| b.id == id) {
            return Err(
                "El identificador reservado ya existe; no se ha sustituido la ficha".into(),
            );
        }
        let m = prepared.metadata;
        books.push(Book {
            id,
            title: m.title,
            author: m.author,
            year: m.year,
            description: m.description,
            tags: m.tags,
            topic: String::new(),
            file_path: None,
            files: vec![prepared.file],
        });
    }
    Ok(id)
}

impl LibraryApp {
    pub fn begin_import(&mut self, paths: Vec<PathBuf>) {
        if self.storage_error.is_some() || self.library_tools.busy() || self.import_flow.active() {
            return;
        }
        if let Some(error) = &self.library_tools.preferences_error {
            self.notice = error.clone();
            return;
        }
        let mut view = import_wizard_view::ImportWizardView::new();
        view.wizard.inputs = paths;
        self.import_flow.wizard = Some(view);
        self.import_flow.generation = self.library_generation;
    }

    fn import_catalog(&self) -> Vec<import_scan::CatalogItem> {
        self.books
            .iter()
            .flat_map(|book| {
                let paths = book.file_paths(&self.library_folder);
                if paths.is_empty() {
                    vec![import_scan::CatalogItem::new(
                        book.id,
                        &book.title,
                        &book.author,
                        None,
                    )]
                } else {
                    paths
                        .into_iter()
                        .map(|path| {
                            import_scan::CatalogItem::new(
                                book.id,
                                &book.title,
                                &book.author,
                                Some(path.to_string_lossy().into_owned()),
                            )
                        })
                        .collect()
                }
            })
            .collect()
    }

    fn execute_import(&mut self, plan: import_wizard::ImportPlan) {
        if self.storage_error.is_some() || self.import_flow.generation != self.library_generation {
            return;
        }
        let mut requests = Vec::new();
        for entry in plan.entries {
            let (id, associate, supplied) = match entry.decision {
                import_wizard::Decision::Skip => continue,
                import_wizard::Decision::Import => {
                    let Some(id) = self.next_book_id() else {
                        return;
                    };
                    (id, false, None)
                }
                import_wizard::Decision::AssociateFormat => {
                    let Some(book) = self
                        .books
                        .iter()
                        .find(|b| Some(b.id) == entry.associate_with)
                    else {
                        self.notice =
                            "La ficha de asociación ya no existe; vuelva a revisar el lote".into();
                        return;
                    };
                    (book.id, true, Some(metadata(book)))
                }
            };
            let source_base = self
                .import_flow
                .wizard
                .as_ref()
                .and_then(|view| {
                    view.wizard
                        .inputs
                        .iter()
                        .find(|base| base.is_dir() && entry.path.starts_with(base))
                })
                .cloned();
            requests.push(import_jobs::ImportRequest {
                book_id: id,
                associate,
                path: entry.path,
                source_base,
                metadata: supplied,
            });
        }
        self.import_flow.wizard = None;
        self.import_flow.results = vec![format!("Omitidos por decisión: {}", plan.to_skip)];
        self.import_flow.results_open = true;
        self.import_flow.progress = (0, requests.len());
        self.import_flow.cancel = Arc::new(AtomicBool::new(false));
        let cancel = Arc::clone(&self.import_flow.cancel);
        let root = self.library_folder.clone();
        let preferences = self.library_tools.preferences.clone();
        let (tx, rx) = mpsc::channel();
        self.import_flow.receiver = Some(rx);
        std::thread::spawn(move || {
            let outcomes = import_jobs::run_batch(
                &requests,
                &root,
                &preferences,
                &cancel,
                crate::read_book_metadata,
                |done, total| {
                    let _ = tx.send(Event::Progress(done, total));
                },
            );
            let _ = tx.send(Event::Done(outcomes));
        });
    }

    pub fn import_ui(&mut self, ctx: &egui::Context) {
        let events: Vec<_> = self
            .import_flow
            .receiver
            .as_ref()
            .map(|rx| rx.try_iter().collect())
            .unwrap_or_default();
        for event in events {
            match event {
                Event::Progress(done, total) => self.import_flow.progress = (done, total),
                Event::Done(outcomes) => {
                    self.import_flow.receiver = None;
                    if self.import_flow.generation != self.library_generation {
                        self.notice = "Resultado de otra biblioteca descartado".into();
                        continue;
                    }
                    let attempted = outcomes.len();
                    let mut imported = 0;
                    for outcome in outcomes {
                        let label = outcome.source.display().to_string();
                        match outcome.result {
                            Ok(prepared) => {
                                let warning = prepared.warning.clone();
                                match apply_prepared(&mut self.books, prepared) {
                                    Ok(id) => {
                                        imported += 1;
                                        self.selected = Some(id);
                                        self.import_flow.results.push(format!("Añadido: {label}"));
                                        if let Some(w) = warning {
                                            self.import_flow.results.push(w);
                                        }
                                    }
                                    Err(error) => {
                                        self.import_flow.results.push(format!("{label}: {error}"))
                                    }
                                }
                            }
                            Err(error) => {
                                self.import_flow.results.push(format!("{label}: {error}"))
                            }
                        }
                    }
                    if imported > 0 {
                        self.save();
                    }
                    let pending = self.import_flow.progress.1.saturating_sub(attempted);
                    self.notice = format!(
                        "Importación: {imported} añadidos, {} con error, {pending} sin procesar",
                        attempted - imported
                    );
                    self.import_flow.results.push(self.notice.clone());
                    if self.storage_error.is_some() {
                        self.import_flow.results.push("Guardado bloqueado: conserve esta ventana abierta y use Reintentar guardado o Exportar cambios pendientes".into());
                    }
                }
            }
        }
        if self.import_flow.wizard.is_some() {
            let catalog = self.import_catalog();
            let mut close = false;
            egui::Window::new("Opciones de importación").show(ctx, |ui| {
                ui.label(format!(
                    "Modo: {:?} · Organización: {:?}",
                    self.library_tools.preferences.import_mode,
                    self.library_tools.preferences.layout
                ));
                ui.label(
                    "Se conservan los originales. La confirmación del plan inicia la importación.",
                );
                close = ui.button("Cerrar asistente sin importar").clicked();
            });
            if close {
                self.import_flow.wizard = None;
            } else if let Some(view) = &mut self.import_flow.wizard {
                view.show(ctx, &catalog);
                if let Some(plan) = view.take_plan() {
                    self.execute_import(plan);
                }
            }
        }
        if self.import_flow.busy() {
            ctx.request_repaint_after(std::time::Duration::from_millis(100));
            egui::Window::new("Importando archivos").collapsible(false).show(ctx, |ui| {
                let (done,total)=self.import_flow.progress;
                ui.label(format!("Procesados {done} de {total}"));
                ui.add(egui::ProgressBar::new(done as f32 / total.max(1) as f32));
                if ui.button("Cancelar después del archivo actual").clicked() {self.import_flow.cancel.store(true,Ordering::Relaxed);}
                ui.weak("Los archivos completados se conservarán. Espere al resultado antes de cerrar.");
            });
        }
        if self.import_flow.results_open {
            egui::Window::new("Resultados de importación")
                .open(&mut self.import_flow.results_open)
                .show(ctx, |ui| {
                    egui::ScrollArea::vertical()
                        .max_height(350.0)
                        .show(ui, |ui| {
                            for result in &self.import_flow.results {
                                ui.label(result);
                            }
                        });
                });
        }
    }
}
