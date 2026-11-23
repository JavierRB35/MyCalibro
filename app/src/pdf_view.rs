//! Independent PDF window. Rendering runs on a worker; persistence belongs to the app.
use crate::pdf_backend::{self, RenderedPdf};
use eframe::egui;
use std::{path::PathBuf, sync::mpsc};

pub struct PdfView {
    pub open: bool,
    pub dark: bool,
    pub zoom: f32,
    book_id: Option<u64>,
    path: Option<PathBuf>,
    page: u32,
    page_count: u32,
    texture: Option<egui::TextureHandle>,
    receiver: Option<mpsc::Receiver<Result<RenderedPdf, String>>>,
    error: Option<String>,
    dirty: bool,
}

impl Default for PdfView {
    fn default() -> Self {
        Self {
            open: false,
            dark: false,
            zoom: 1.0,
            book_id: None,
            path: None,
            page: 0,
            page_count: 0,
            texture: None,
            receiver: None,
            error: None,
            dirty: false,
        }
    }
}

impl PdfView {
    /// Opening another book drops the old receiver, so stale pages cannot enter the new book.
    pub fn open_path(
        &mut self,
        ctx: &egui::Context,
        path: PathBuf,
        book_id: u64,
        saved_page: Option<u32>,
    ) {
        self.book_id = Some(book_id);
        self.path = Some(path);
        self.page = saved_page.unwrap_or(0);
        self.page_count = 0;
        self.texture = None;
        self.receiver = None;
        self.dirty = false;
        self.open = true;
        self.render(ctx, self.page);
    }

    /// Keep this pending until the owning library has successfully persisted it.
    pub fn pending_position(&self) -> Option<(u64, u32)> {
        if self.dirty {
            self.book_id.map(|id| (id, self.page))
        } else {
            None
        }
    }
    pub fn mark_saved(&mut self) {
        self.dirty = false;
    }

    /// Call when switching libraries, before loading the new library's reader state.
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    fn render(&mut self, ctx: &egui::Context, page: u32) {
        let Some(path) = self.path.clone() else {
            return;
        };
        let width = (1024.0 * self.zoom).round() as u32;
        let (tx, rx) = mpsc::channel();
        self.receiver = Some(rx);
        self.error = None;
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let _ = tx.send(pdf_backend::render_page(&path, page, width));
            ctx.request_repaint();
        });
    }

    pub fn show(&mut self, ctx: &egui::Context) {
        if let Some(receiver) = &self.receiver {
            match receiver.try_recv() {
                Ok(result) => {
                    self.receiver = None;
                    match result.and_then(|page| {
                        image::load_from_memory(&page.png)
                            .map(|image| (page, image.to_rgba8()))
                            .map_err(|e| format!("La página PDF no se pudo dibujar: {e}"))
                    }) {
                        Ok((page, pixels)) => {
                            self.page = page.page_index;
                            self.page_count = page.page_count;
                            let image = egui::ColorImage::from_rgba_unmultiplied(
                                [pixels.width() as usize, pixels.height() as usize],
                                pixels.as_raw(),
                            );
                            self.texture = Some(ctx.load_texture(
                                "pdf-current-page",
                                image,
                                egui::TextureOptions::LINEAR,
                            ));
                            self.dirty = true;
                        }
                        Err(error) => self.error = Some(error),
                    }
                }
                Err(mpsc::TryRecvError::Empty) => {
                    ctx.request_repaint_after(std::time::Duration::from_millis(100))
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.receiver = None;
                    self.error = Some("El motor PDF terminó sin devolver una página".into());
                }
            }
        }
        if !self.open {
            return;
        }
        let mut open = self.open;
        egui::Window::new("Lector PDF")
            .open(&mut open)
            .default_width(900.0)
            .default_height(680.0)
            .show(ctx, |ui| {
                if let Some(path) = &self.path {
                    ui.strong(path.file_name().unwrap_or_default().to_string_lossy());
                }
                let busy = self.receiver.is_some();
                ui.horizontal_wrapped(|ui| {
                    if ui
                        .add_enabled(!busy && self.page > 0, egui::Button::new("Anterior"))
                        .clicked()
                    {
                        self.render(ctx, self.page - 1);
                    }
                    let mut requested = self.page.saturating_add(1);
                    let response = ui.add_enabled(
                        !busy && self.page_count > 0,
                        egui::DragValue::new(&mut requested)
                            .range(1..=self.page_count.max(1))
                            .prefix("Página "),
                    );
                    ui.label(format!("de {}", self.page_count));
                    if response.changed() {
                        self.render(ctx, requested.saturating_sub(1));
                    }
                    if ui
                        .add_enabled(
                            !busy && self.page + 1 < self.page_count,
                            egui::Button::new("Siguiente"),
                        )
                        .clicked()
                    {
                        self.render(ctx, self.page + 1);
                    }
                    let zoom_response = ui.add_enabled(
                        !busy,
                        egui::Slider::new(&mut self.zoom, 0.5..=2.0).text("Zoom"),
                    );
                    if zoom_response.drag_stopped()
                        || (zoom_response.changed() && !zoom_response.dragged())
                    {
                        self.render(ctx, self.page);
                    }
                    if ui.checkbox(&mut self.dark, "Entorno oscuro").changed() {
                        self.dirty = true;
                    }
                });
                if busy {
                    ui.spinner();
                    ui.label("Preparando página…");
                }
                if let Some(error) = &self.error {
                    ui.colored_label(egui::Color32::DARK_RED, error);
                }
                let background = if self.dark {
                    egui::Color32::from_gray(28)
                } else {
                    egui::Color32::from_gray(245)
                };
                egui::Frame::new().fill(background).show(ui, |ui| {
                    egui::ScrollArea::both().auto_shrink(false).show(ui, |ui| {
                        if let Some(texture) = &self.texture {
                            let size = texture.size_vec2();
                            let scale = ui.available_width().max(100.0) / size.x * self.zoom;
                            ui.image((texture.id(), size * scale));
                        }
                    });
                });
            });
        self.open = open;
    }
}
