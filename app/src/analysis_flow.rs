//! On-demand analysis UI; workers consult the live permission manager on every send.
use crate::{
    ai_consent::{ConsentStatus, TextRequest, CONSENT_SCOPE_VERSION},
    analysis_backend::{self, AnalysisKind, AnalysisRequest, AnalysisResult, ApiKeys},
};
use eframe::egui;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    mpsc, Arc,
};

pub struct AnalysisFlow {
    book_id: Option<u64>,
    kind: AnalysisKind,
    question: String,
    use_content: bool,
    rx: Option<mpsc::Receiver<Result<AnalysisResult, String>>>,
    cancel: Arc<AtomicBool>,
    result: Option<AnalysisResult>,
    error: Option<String>,
}
impl Default for AnalysisFlow {
    fn default() -> Self {
        Self {
            book_id: None,
            kind: AnalysisKind::Summary,
            question: String::new(),
            use_content: false,
            rx: None,
            cancel: Arc::new(AtomicBool::new(false)),
            result: None,
            error: None,
        }
    }
}
impl AnalysisFlow {
    pub fn reset(&mut self) {
        self.cancel.store(true, Ordering::SeqCst);
        *self = Self::default();
    }
    pub fn open(&mut self, id: u64) {
        if self.book_id != Some(id) {
            self.reset();
        }
        self.book_id = Some(id);
    }
}
impl crate::LibraryApp {
    pub fn analysis_ui(&mut self, ctx: &egui::Context) {
        let Some(id) = self.analysis_flow.book_id else {
            return;
        };
        let Some(book) = self.books.iter().find(|b| b.id == id).cloned() else {
            self.analysis_flow.reset();
            return;
        };
        if let Some(rx) = &self.analysis_flow.rx {
            match rx.try_recv() {
                Ok(result) => {
                    self.analysis_flow.rx = None;
                    match result {
                        Ok(r) => self.analysis_flow.result = Some(r),
                        Err(e) => self.analysis_flow.error = Some(e),
                    }
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.analysis_flow.rx = None;
                    self.analysis_flow.error = Some("El análisis se interrumpió".into());
                }
                Err(mpsc::TryRecvError::Empty) => (),
            }
        }
        let mut open = true;
        let mut run = false;
        let mut authorize = false;
        egui::Window::new(format!("Análisis · {}",book.title)).id(egui::Id::new("analysis-window")).open(&mut open).default_width(550.0).vscroll(true).show(ctx,|ui|{
            let flow=&mut self.analysis_flow;
            ui.label("Google Gemini · respaldo Groq. Se envían título, autor y descripción; contenido solo con autorización.");
            ui.add_enabled_ui(flow.rx.is_none(),|ui|{
                ui.horizontal(|ui|{ui.selectable_value(&mut flow.kind,AnalysisKind::Summary,"Resumen");ui.selectable_value(&mut flow.kind,AnalysisKind::Analysis,"Análisis");ui.selectable_value(&mut flow.kind,AnalysisKind::Question,"Pregunta");});
                if flow.kind==AnalysisKind::Question {ui.text_edit_multiline(&mut flow.question);}
                ui.checkbox(&mut flow.use_content,"Usar fragmentos del contenido (hasta 6000 caracteres)");
                if !flow.use_content {ui.weak("Solo metadatos: no se abre ni se envía el archivo del libro.");}
                if flow.use_content && matches!(self.consents.read().unwrap().status(id),ConsentStatus::MetadataOnly) {
                    authorize=ui.button("Revisar autorización de contenido…").clicked();
                }
                run=ui.add_enabled(self.aux_error.is_none(),egui::Button::new("Consultar IA")).clicked();
            });
            if flow.rx.is_some() {ui.spinner(); ui.label("Analizando…"); if ui.button("Cancelar consulta").clicked() {flow.cancel.store(true,Ordering::SeqCst);flow.rx=None;flow.error=Some("Consulta cancelada; una petición ya enviada no puede retirarse".into());}}
            if let Some(error)=&flow.error {ui.colored_label(egui::Color32::DARK_RED,error);}
            if let Some(result)=&flow.result {ui.separator();ui.strong(result.source.label());ui.weak(result.provider.display_name());ui.label(&result.context_note);ui.label(&result.answer);for citation in &result.citations {ui.separator();ui.strong(format!("{} · {}",citation.fragment_id,citation.location));ui.label(&citation.quote);}}
        });
        if !open {
            self.analysis_flow.reset();
            return;
        }
        if authorize {
            self.consent_view.request_consent(id, book.title.clone());
        }
        if run {
            let flow = &mut self.analysis_flow;
            flow.result = None;
            flow.error = None;
            if flow.use_content
                && matches!(
                    self.consents.read().unwrap().status(id),
                    ConsentStatus::MetadataOnly
                )
            {
                self.consent_view.request_consent(id, book.title);
                flow.error = Some(
                    "Autoriza el envío y vuelve a pulsar Consultar IA, o usa solo metadatos".into(),
                );
                return;
            }
            let source_path = book
                .file_paths(&self.library_folder)
                .into_iter()
                .find(|p| p.is_file());
            let request = AnalysisRequest {
                book_id: id,
                title: book.title,
                author: book.author,
                description: book.description,
                kind: flow.kind,
                question: flow.question.clone(),
                use_content: flow.use_content,
                source_path,
                ebook_convert: crate::find_ebook_converter(),
            };
            let keys = ApiKeys {
                gemini: crate::load_api_key("Gemini"),
                groq: crate::load_api_key("Groq"),
            };
            flow.cancel = Arc::new(AtomicBool::new(false));
            let cancel = flow.cancel.clone();
            let consents = self.consents.clone();
            let ctx = ctx.clone();
            let (tx, rx) = mpsc::channel();
            flow.rx = Some(rx);
            std::thread::spawn(move || {
                let result = analysis_backend::analyze(&request, &keys, |provider, chars| {
                    if cancel.load(Ordering::SeqCst) {
                        return Err("Consulta cancelada o biblioteca cambiada".into());
                    }
                    if chars > 0 {
                        consents
                            .read()
                            .map_err(|_| "Permisos no disponibles")?
                            .can_send_text(
                                request.book_id,
                                TextRequest {
                                    provider,
                                    chars_to_send: chars,
                                    scope_version: CONSENT_SCOPE_VERSION,
                                },
                            )
                            .map_err(|_| {
                                "Autorización de contenido ausente, revocada o fuera de alcance"
                            })?;
                    }
                    Ok(())
                });
                if !cancel.load(Ordering::SeqCst) {
                    let _ = tx.send(result);
                }
                ctx.request_repaint();
            });
        }
    }
}
