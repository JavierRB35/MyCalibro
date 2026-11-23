//! Vista egui del asistente de importación masiva (PROD-010).
//!
//! Solo muestra y muta [`ImportWizard`]; toda la lógica vive en los módulos
//! puros `import_scan` e `import_wizard`. El escaneo corre en un hilo
//! trabajador y comunica progreso y resultado por un canal `mpsc`, para no
//! bloquear la interfaz.
//!
//! Contrato con la aplicación (integración en main.rs): cada fotograma la
//! vista llama a `show(ctx, catálogo)`; cuando el usuario confirma el plan,
//! `take_plan()` lo devuelve una sola vez para que el backend lo ejecute
//! (crear fichas, asociar formatos). La vista nunca escribe el catálogo.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, Receiver};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use eframe::egui;

use crate::import_scan::{
    scan_paths_with_progress, CatalogItem, DuplicateMatch, MatchKind, ScanOutcome,
};
use crate::import_wizard::{Decision, ImportPlan, ImportWizard, Phase, Summary};

/// Mensajes del hilo trabajador hacia la interfaz.
enum WorkerMsg {
    Progress { files: usize, candidates: usize },
    Done(Box<ScanOutcome>),
}

/// Estado de la vista del asistente, incluida la comunicación con el hilo.
pub struct ImportWizardView {
    pub wizard: ImportWizard,
    rx: Option<Receiver<WorkerMsg>>,
    plan: Option<ImportPlan>,
    mensaje: Option<String>,
    /// Bandera de cancelación cooperativa: el trabajador la consulta en cada
    /// tick de progreso y aborta el recorrido cuanto antes (hallazgo del
    /// AGENTE A: cerrar el canal solo hace que el resultado se descarte).
    cancel: Arc<AtomicBool>,
}

impl ImportWizardView {
    pub fn new() -> Self {
        Self {
            wizard: ImportWizard::new(),
            rx: None,
            plan: None,
            mensaje: None,
            cancel: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Plan confirmado por el usuario, listo para que el backend lo ejecute.
    pub fn take_plan(&mut self) -> Option<ImportPlan> {
        self.plan.take()
    }

    /// Añade archivos elegidos con el diálogo del sistema (solo en fase Idle).
    pub fn add_files(&mut self) {
        if self.wizard.phase != Phase::Idle {
            return;
        }
        if let Some(files) = rfd::FileDialog::new()
            .add_filter("Libros", crate::import_scan::SUPPORTED_BOOK_EXTENSIONS)
            .pick_files()
        {
            self.wizard.inputs.extend(files);
        }
    }

    /// Añade una carpeta que se explorará recursivamente (solo en fase Idle).
    pub fn add_folder(&mut self) {
        if self.wizard.phase != Phase::Idle {
            return;
        }
        if let Some(folder) = rfd::FileDialog::new().pick_folder() {
            self.wizard.inputs.push(folder);
        }
    }

    /// Quita una ruta de la lista de entradas.
    pub fn remove_input(&mut self, index: usize) {
        if self.wizard.phase == Phase::Idle && index < self.wizard.inputs.len() {
            self.wizard.inputs.remove(index);
        }
    }

    /// Lanza el escaneo en un hilo trabajador con el catálogo actual. Si el
    /// usuario cancela, el receptor se suelta y el trabajador termina cuando
    /// el canal se cierra.
    pub fn begin_scan(&mut self, catalog: &[CatalogItem]) {
        if self.wizard.phase != Phase::Idle || self.wizard.inputs.is_empty() {
            return;
        }
        let (tx, rx) = channel::<WorkerMsg>();
        self.rx = Some(rx);
        self.mensaje = None;
        self.cancel.store(false, Ordering::Relaxed);
        let cancel = Arc::clone(&self.cancel);
        let inputs = self.wizard.inputs.clone();
        let catalog = catalog.to_vec();
        self.wizard.start_scan(inputs.clone());
        thread::spawn(move || {
            let result = scan_paths_with_progress(&inputs, &catalog, &mut |files, candidates| {
                let cancelado = cancel.load(Ordering::Relaxed);
                if !cancelado {
                    // Un error al enviar solo significa que se soltó el canal.
                    let _ = tx.send(WorkerMsg::Progress { files, candidates });
                }
                !cancelado // cooperativo: detiene el recorrido en cuanto marque
            });
            if !cancel.load(Ordering::Relaxed) {
                let _ = tx.send(WorkerMsg::Done(Box::new(result)));
            }
        });
    }

    /// Consume los mensajes pendientes del trabajador. Devuelve true si el
    /// escaneo sigue en curso (para pedir repintado).
    pub fn poll(&mut self, ctx: &egui::Context) -> bool {
        let mut scanning = self.wizard.phase == Phase::Scanning;
        if let Some(rx) = &self.rx {
            let mut terminado = false;
            while let Ok(msg) = rx.try_recv() {
                match msg {
                    WorkerMsg::Progress { files, candidates } => {
                        self.wizard.record_progress(files, candidates);
                    }
                    WorkerMsg::Done(outcome) => {
                        self.wizard.finish_scan(*outcome);
                        terminado = true;
                    }
                }
            }
            if terminado {
                self.rx = None;
                scanning = false;
            }
        }
        if scanning {
            ctx.request_repaint_after(Duration::from_millis(100));
        }
        scanning
    }

    /// Dibuja la ventana del asistente. Debe llamarse cada fotograma con el
    /// catálogo actual para que el análisis de duplicados use datos vivos.
    pub fn show(&mut self, ctx: &egui::Context, catalog: &[CatalogItem]) {
        self.poll(ctx);
        egui::Window::new("Importar libros")
            .default_width(760.0)
            .default_height(520.0)
            .collapsible(false)
            .show(ctx, |ui| match self.wizard.phase {
                Phase::Idle => self.draw_idle(ui, catalog),
                Phase::Scanning => self.draw_scanning(ui),
                Phase::Reviewing => self.draw_review(ui),
            });
        if let Some(mensaje) = &self.mensaje {
            let mensaje = mensaje.clone();
            egui::Window::new("Importar libros")
                .default_width(760.0)
                .collapsible(false)
                .title_bar(false)
                .show(ctx, |ui| {
                    ui.colored_label(egui::Color32::DARK_RED, mensaje);
                });
        }
    }

    fn draw_idle(&mut self, ui: &mut egui::Ui, catalog: &[CatalogItem]) {
        ui.label(
            "Selecciona archivos o carpetas de libros. Las carpetas se exploran \
             de forma recursiva y, antes de importar nada, podrás revisar los \
             posibles duplicados.",
        );
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            if ui.button("Añadir archivos…").clicked() {
                self.add_files();
            }
            if ui.button("Añadir carpeta…").clicked() {
                self.add_folder();
            }
        });
        ui.add_space(4.0);
        if self.wizard.inputs.is_empty() {
            ui.weak("Todavía no has añadido archivos ni carpetas.");
        } else {
            egui::ScrollArea::vertical()
                .max_height(160.0)
                .show(ui, |ui| {
                    for (index, ruta) in self.wizard.inputs.clone().iter().enumerate() {
                        ui.horizontal(|ui| {
                            ui.label(ruta.display().to_string());
                            if ui.small_button("Quitar").clicked() {
                                self.remove_input(index);
                            }
                        });
                    }
                });
        }
        ui.add_space(8.0);
        ui.add_enabled_ui(!self.wizard.inputs.is_empty(), |ui| {
            if ui.button("Buscar libros").clicked() {
                self.begin_scan(catalog);
            }
        });
    }

    fn draw_scanning(&mut self, ui: &mut egui::Ui) {
        ui.spinner();
        ui.label(format!(
            "Buscando libros… {} archivos vistos, {} candidatos hasta ahora.",
            self.wizard.progress.files_seen, self.wizard.progress.candidates_found
        ));
        ui.weak("Puedes cancelar; no se importará nada sin tu revisión.");
        if ui.button("Cancelar búsqueda").clicked() {
            self.wizard.cancel_scan();
            self.cancel.store(true, Ordering::Relaxed);
            self.rx = None; // el trabajador se detiene y descarta el resultado
        }
    }

    fn draw_review(&mut self, ui: &mut egui::Ui) {
        let summary = self.wizard.summary();
        ui.label(format!(
            "{} candidatos encontrados. Revisa los avisos y decide qué hacer con cada uno.",
            summary.candidates
        ));
        ui.add_space(6.0);
        let reviews = self
            .wizard
            .outcome
            .as_ref()
            .map(|outcome| outcome.reviews.clone())
            .unwrap_or_default();
        egui::ScrollArea::vertical()
            .auto_shrink(false)
            .show(ui, |ui| {
                for (index, review) in reviews.iter().enumerate() {
                    ui.push_id(index, |ui| {
                        ui.separator();
                        ui.strong(
                            review
                                .candidate
                                .path
                                .file_name()
                                .map(|name| name.to_string_lossy().to_string())
                                .unwrap_or_else(|| review.candidate.path.display().to_string()),
                        );
                        ui.weak(format!(
                            "{} · {}",
                            review.candidate.path.display(),
                            formato_tamano(review.candidate.size_bytes)
                        ));
                        if let Some(coincidencia) =
                            review.matches.iter().max_by_key(|m| m.certainty)
                        {
                            let (color, texto) = aviso_coincidencia(coincidencia);
                            ui.colored_label(color, texto);
                        }
                        let mut actual = self.wizard.decision(index).unwrap_or(Decision::Skip);
                        ui.horizontal(|ui| {
                            ui.radio_value(&mut actual, Decision::Import, "Importar");
                            ui.radio_value(&mut actual, Decision::Skip, "Omitir");
                            ui.radio_value(
                                &mut actual,
                                Decision::AssociateFormat,
                                "Asociar formato",
                            );
                        });
                        self.wizard.set_decision(index, actual);
                    });
                }
            });
        ui.add_space(8.0);
        self.draw_footer(ui, summary);
    }

    fn draw_footer(&mut self, ui: &mut egui::Ui, summary: Summary) {
        ui.horizontal(|ui| {
            ui.label(format!(
                "Importar: {} · Omitir: {} · Asociar: {} · Sin decidir: {} · Con avisos: {}",
                summary.to_import,
                summary.to_skip,
                summary.to_associate,
                summary.undecided,
                summary.with_warnings
            ));
            if ui.button("Aplicar sugerencias").clicked() {
                self.wizard.apply_suggestions();
            }
        });
        ui.add_space(4.0);
        if ui.button("Generar plan de importación").clicked() {
            match self.wizard.build_plan() {
                Ok(plan) => {
                    self.mensaje = None;
                    self.plan = Some(plan);
                }
                Err(error) => self.mensaje = Some(error),
            }
        }
        if self.plan.is_some() {
            ui.colored_label(
                egui::Color32::from_rgb(48, 111, 91),
                "Plan listo. La aplicación lo ejecutará al cerrar este asistente.",
            );
        }
        if let Some(errores) = self
            .wizard
            .outcome
            .as_ref()
            .filter(|outcome| !outcome.errors.is_empty())
        {
            ui.add_space(4.0);
            egui::CollapsingHeader::new(format!("Errores del escaneo ({})", errores.errors.len()))
                .show(ui, |ui| {
                    for error in &errores.errors {
                        ui.weak(format!("{}: {}", error.path.display(), error.message));
                    }
                });
        }
    }
}

impl Default for ImportWizardView {
    fn default() -> Self {
        Self::new()
    }
}

/// Texto y color del aviso según la coincidencia más fuerte; explica al
/// usuario qué se ha comparado y con qué certeza (PROD-010).
fn aviso_coincidencia(coincidencia: &DuplicateMatch) -> (egui::Color32, String) {
    let libro = format!(
        "«{}» de {}",
        coincidencia.title,
        if coincidencia.author.is_empty() {
            "autor desconocido"
        } else {
            &coincidencia.author
        }
    );
    match coincidencia.kind {
        MatchKind::ExactPath => (
            egui::Color32::DARK_RED,
            format!(
                "Ruta idéntica a la del libro {} registrado. Probablemente es el mismo archivo.",
                libro
            ),
        ),
        MatchKind::SameFileName => (
            egui::Color32::from_rgb(172, 106, 0),
            format!(
                "Mismo nombre de archivo que {} ({}) en otra carpeta.",
                libro,
                coincidencia.path.as_deref().unwrap_or("ruta no registrada")
            ),
        ),
        MatchKind::TitleAuthorSimilar => (
            egui::Color32::from_rgb(120, 120, 30),
            format!(
                "Título y autor parecidos a {}. Comprueba antes de importar.",
                libro
            ),
        ),
    }
}

/// Tamaño legible en KiB/MiB para la lista de candidatos.
fn formato_tamano(bytes: u64) -> String {
    const KIB: f64 = 1024.0;
    const MIB: f64 = 1024.0 * 1024.0;
    if bytes as f64 >= MIB {
        format!("{:.1} MiB", bytes as f64 / MIB)
    } else {
        format!("{:.0} KiB", bytes as f64 / KIB)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::import_scan::scan_paths;

    /// La vista no puede ejecutarse sin ventana, pero su lógica no egui sí se
    /// puede comprobar: el canal trabajador→vista actualiza el estado y el
    /// plan se entrega una sola vez.
    #[test]
    fn el_canal_del_trabajador_actualiza_el_estado_del_asistente() {
        let base = std::env::temp_dir().join(format!("mycalibro_view_{}", std::process::id()));
        std::fs::create_dir_all(&base).unwrap();
        std::fs::write(base.join("libro.epub"), b"x").unwrap();
        let mut view = ImportWizardView::new();
        let (tx, rx) = channel::<WorkerMsg>();
        view.rx = Some(rx);
        view.wizard.start_scan(vec![base.clone()]);
        tx.send(WorkerMsg::Progress {
            files: 3,
            candidates: 1,
        })
        .unwrap();
        let outcome = scan_paths(&[base.clone()], &[]);
        tx.send(WorkerMsg::Done(Box::new(outcome))).unwrap();
        drop(tx);
        let ctx = egui::Context::default();
        assert!(!view.poll(&ctx), "el escaneo ya terminó");
        assert_eq!(view.wizard.phase, Phase::Reviewing);
        assert_eq!(
            view.wizard.progress,
            crate::import_wizard::ScanProgress {
                files_seen: 3,
                candidates_found: 1
            }
        );
        assert_eq!(view.wizard.summary().candidates, 1);
        std::fs::remove_dir_all(&base).unwrap();
    }

    #[test]
    fn el_plan_se_entrega_solo_tras_confirmarse_y_una_vez() {
        let mut view = ImportWizardView::new();
        assert!(view.take_plan().is_none());
        let base = std::env::temp_dir().join(format!("mycalibro_viewplan_{}", std::process::id()));
        std::fs::create_dir_all(&base).unwrap();
        std::fs::write(base.join("libro.epub"), b"x").unwrap();
        let outcome = scan_paths(&[base.clone()], &[]);
        view.wizard.finish_scan(outcome);
        // Sin duplicados: plan válido directo.
        let plan = view
            .wizard
            .build_plan()
            .expect("lote simple sin duplicados");
        view.plan = Some(plan);
        let entregado = view.take_plan().expect("primer take entrega el plan");
        assert_eq!(entregado.entries.len(), 1);
        assert!(
            view.take_plan().is_none(),
            "un plan solo se entrega una vez"
        );
        std::fs::remove_dir_all(&base).unwrap();
    }

    #[test]
    fn avisos_y_tamanos_son_legibles_para_el_usuario() {
        let coincidencia = DuplicateMatch {
            catalog_id: 1,
            title: "1984".to_owned(),
            author: "George Orwell".to_owned(),
            path: None,
            kind: MatchKind::ExactPath,
            certainty: crate::import_scan::Certainty::High,
        };
        let (color, texto) = aviso_coincidencia(&coincidencia);
        assert!(texto.contains("1984"));
        assert!(texto.contains("Ruta idéntica"));
        assert_eq!(color, egui::Color32::DARK_RED);
        assert_eq!(formato_tamano(0), "0 KiB");
        assert_eq!(formato_tamano(2048), "2 KiB");
        assert_eq!(formato_tamano(3 * 1024 * 1024), "3.0 MiB");
    }
}
