use crate::{
    backup,
    storage::{Layout, Preferences, StorageMode},
    Book,
};
use eframe::egui;
use std::{
    path::{Path, PathBuf},
    sync::mpsc,
};

enum Outcome {
    Backup(backup::BackupReport),
    Verified(Vec<String>),
    Restored(backup::BackupReport),
    Exported(PathBuf),
}

pub struct LibraryTools {
    pub open: bool,
    pub preferences: Preferences,
    pub preferences_error: Option<String>,
    status: String,
    receiver: Option<mpsc::Receiver<Result<Outcome, String>>>,
}

impl LibraryTools {
    pub fn recover_catalog(&mut self, ctx: &egui::Context, source: PathBuf, destination: PathBuf) {
        if self.busy() {
            return;
        }
        self.open = true;
        self.start(ctx,move || {
            crate::persistence::recover_into(&source,&destination)?;
            Ok(Outcome::Restored(backup::BackupReport {path:destination,exclusions:vec!["Recuperación de catálogo y recursos administrados; no incluye preferencias ni estado auxiliar. Los vínculos externos conservan su origen".into()]}))
        });
    }

    pub fn export_pending(
        &mut self,
        ctx: &egui::Context,
        books: Vec<Book>,
        root: PathBuf,
        destination: PathBuf,
    ) {
        if self.busy() {
            return;
        }
        self.open = true;
        self.start(ctx, move || {
            crate::persistence::recover_books_into(&books, &root, &destination)?;
            Ok(Outcome::Exported(destination))
        });
    }

    pub fn new(root: &Path) -> Self {
        let (preferences, preferences_error) = match Preferences::load(root) {
            Ok(p) => (p, None),
            Err(e) => (Preferences::default(), Some(e)),
        };
        Self {
            open: false,
            preferences,
            preferences_error,
            status: String::new(),
            receiver: None,
        }
    }

    fn start(
        &mut self,
        ctx: &egui::Context,
        operation: impl FnOnce() -> Result<Outcome, String> + Send + 'static,
    ) {
        let (sender, receiver) = mpsc::channel();
        self.receiver = Some(receiver);
        self.status = "Operación en curso…".into();
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let _ = sender.send(operation());
            ctx.request_repaint();
        });
    }

    pub fn show(
        &mut self,
        ctx: &egui::Context,
        root: &Path,
        books: &[Book],
        writable: bool,
    ) -> Option<PathBuf> {
        let mut restored = None;
        if let Some(result) = self.receiver.as_ref().and_then(|r| r.try_recv().ok()) {
            self.receiver = None;
            match result {
                Ok(Outcome::Backup(report)) => {
                    self.status = report_text("Respaldo creado", &report)
                }
                Ok(Outcome::Verified(exclusions)) => {
                    self.status = if exclusions.is_empty() {
                        "Integridad verificada; sin exclusiones registradas".into()
                    } else {
                        format!(
                            "Integridad verificada, con exclusiones:\n{}",
                            exclusions.join("\n")
                        )
                    }
                }
                Ok(Outcome::Restored(report)) => {
                    self.status = report_text("Restauración creada", &report);
                    restored = Some(report.path);
                }
                Ok(Outcome::Exported(path))=>self.status=format!("Cambios pendientes y recursos administrados exportados a {}. Origen conservado; vínculos externos sin copiar",path.display()),
                Err(e) => self.status = format!("No se completó la operación: {e}"),
            }
        }
        if !self.open {
            return restored;
        }
        let mut open = self.open;
        egui::Window::new("Biblioteca y copias de seguridad").open(&mut open).resizable(true).show(ctx,|ui| {
            ui.label(format!("Carpeta: {}",root.display()));
            if let Some(error)=&self.preferences_error { ui.colored_label(egui::Color32::DARK_RED,error); }
            let available=writable && self.receiver.is_none();
            if self.preferences_error.is_some() && ui.add_enabled(available,egui::Button::new("Restablecer preferencias conservando copia…")).clicked() {
                match Preferences::reset_preserving(root) {
                    Ok(path) => {
                        self.preferences=Preferences::default();self.preferences_error=None;
                        self.status=path.map(|p|format!("Preferencias restablecidas; original conservado en {}",p.display())).unwrap_or_else(||"Preferencias restablecidas".into());
                    }
                    Err(error)=>self.status=error,
                }
            }
            ui.add_enabled_ui(available && self.preferences_error.is_none(),|ui| {
                let previous=serde_json::to_string(&self.preferences).unwrap_or_default();
                ui.label("Al importar libros:");
                ui.radio_value(&mut self.preferences.import_mode,StorageMode::Linked,"Vincular al archivo original (no copiar)");
                ui.radio_value(&mut self.preferences.import_mode,StorageMode::Managed,"Copiar a la biblioteca administrada");
                ui.label("Organización de copias administradas:");
                ui.radio_value(&mut self.preferences.layout,Layout::Preserve,"Conservar nombres y estructura de origen");
                ui.radio_value(&mut self.preferences.layout,Layout::AuthorTitle,"Carpetas por autor y obra");
                ui.checkbox(&mut self.preferences.include_external_in_backup,"Incluir archivos vinculados en los respaldos");
                if serde_json::to_string(&self.preferences).unwrap_or_default()!=previous {
                    if let Err(error)=self.preferences.save(root) {self.preferences_error=Some(error);}
                }
            });
            ui.separator();
            ui.label("El respaldo incluye catálogo, archivos administrados y preferencias de esta biblioteca. Los externos excluidos o inaccesibles se registran en el manifiesto.");
            ui.add_enabled_ui(available,|ui| {
                if ui.button("Crear copia de seguridad…").clicked() {
                    if let Some(destination)=rfd::FileDialog::new().set_title("Destino fuera de la biblioteca").pick_folder() {
                        match serde_json::to_vec_pretty(books) {
                            Ok(catalog) => {let root=root.to_owned();let include=self.preferences.include_external_in_backup;self.start(ctx,move || backup::create(&root,&catalog,&destination,include).map(Outcome::Backup));},
                            Err(e) => self.status=e.to_string(),
                        }
                    }
                }
                if ui.button("Verificar copia…").clicked() {
                    if let Some(source)=rfd::FileDialog::new().set_title("Carpeta con manifest.json").pick_folder() {self.start(ctx,move || backup::verify(&source).map(Outcome::Verified));}
                }
                if ui.button("Restaurar en una biblioteca nueva…").clicked() {
                    if let Some(source)=rfd::FileDialog::new().set_title("Carpeta con manifest.json").pick_folder() {
                        if let Some(destination)=rfd::FileDialog::new().set_title("Carpeta donde crear la biblioteca recuperada").pick_folder() {self.start(ctx,move || backup::restore(&source,&destination).map(Outcome::Restored));}
                    }
                }
            });
            if self.receiver.is_some() {ui.spinner();ui.label("Puede seguir consultando la biblioteca. El cierre no completa una operación pendiente.");}
            ui.label(&self.status);
        });
        self.open = open;
        restored
    }

    pub fn status(&self) -> &str {
        &self.status
    }

    pub fn busy(&self) -> bool {
        self.receiver.is_some()
    }
}

fn report_text(prefix: &str, report: &backup::BackupReport) -> String {
    if report.exclusions.is_empty() {
        format!("{prefix}: {}. Sin exclusiones.", report.path.display())
    } else {
        format!(
            "{prefix}: {}. No incluye todos los libros:\n{}",
            report.path.display(),
            report.exclusions.join("\n")
        )
    }
}
