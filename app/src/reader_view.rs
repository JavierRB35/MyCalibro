//! Vista egui del lector básico EPUB (PROD-004).
//!
//! Solo muestra y muta la lógica de [`crate::reader_epub`]: navegación por
//! capítulos y páginas, preferencias básicas de visualización (tamaño de
//! letra, modo claro/oscuro) y recuperación de la última posición.
//!
//! Alcance expresamente excluido por PROD-004 y por tanto ausente aquí:
//! subrayados, notas, marcadores avanzados, estadísticas, OCR e IA dentro
//! del lector.
//!
//! Contrato con la aplicación (integración en main.rs): la app llama a
//! `open_path(ruta, book_id, posición_guardada)` y cada fotograma a
//! `show(ctx)`; cuando la posición cambia (`position_dirty()`), la app
//! persiste lo que devuelve `take_position()` usando su capa de
//! almacenamiento. Los errores de apertura se muestran tal cual; nunca se
//! simula contenido.

use std::path::Path;

use eframe::egui;

use crate::reader_epub::{EpubDocument, ReadingPosition};

/// Tamaños de letra del lector; el factor se aplica al tamaño base.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FontScale {
    Small,
    Normal,
    Large,
    ExtraLarge,
}

impl FontScale {
    pub fn factor(self) -> f32 {
        match self {
            FontScale::Small => 0.85,
            FontScale::Normal => 1.0,
            FontScale::Large => 1.25,
            FontScale::ExtraLarge => 1.5,
        }
    }

    /// Párrafos por página: a letra más grande, menos párrafos por página.
    pub fn page_size(self) -> usize {
        ((14.0 / self.factor()).round() as usize).clamp(8, 32)
    }

    pub fn cycle(self) -> Self {
        match self {
            FontScale::Small => FontScale::Normal,
            FontScale::Normal => FontScale::Large,
            FontScale::Large => FontScale::ExtraLarge,
            FontScale::ExtraLarge => FontScale::Small,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            FontScale::Small => "Pequeño",
            FontScale::Normal => "Normal",
            FontScale::Large => "Grande",
            FontScale::ExtraLarge => "Muy grande",
        }
    }
}

/// Tema visual del lector; los colores son locales a la vista para no
/// alterar el tema global de la aplicación.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReaderTheme {
    Light,
    Dark,
}

impl ReaderTheme {
    pub fn toggle(self) -> Self {
        match self {
            ReaderTheme::Light => ReaderTheme::Dark,
            ReaderTheme::Dark => ReaderTheme::Light,
        }
    }

    fn background(self) -> egui::Color32 {
        match self {
            ReaderTheme::Light => egui::Color32::from_rgb(247, 247, 243),
            ReaderTheme::Dark => egui::Color32::from_rgb(30, 32, 34),
        }
    }

    fn text(self) -> egui::Color32 {
        match self {
            ReaderTheme::Light => egui::Color32::from_rgb(39, 49, 47),
            ReaderTheme::Dark => egui::Color32::from_rgb(222, 224, 219),
        }
    }

    fn muted(self) -> egui::Color32 {
        match self {
            ReaderTheme::Light => egui::Color32::from_rgb(125, 133, 129),
            ReaderTheme::Dark => egui::Color32::from_rgb(150, 155, 150),
        }
    }

    fn label(self) -> &'static str {
        match self {
            ReaderTheme::Light => "Claro",
            ReaderTheme::Dark => "Oscuro",
        }
    }
}

/// Estado completo de la vista del lector.
pub struct ReaderView {
    pub document: Option<EpubDocument>,
    /// Identificador estable del libro en el catálogo, para la posición.
    pub book_id: Option<u64>,
    pub position: ReadingPosition,
    pub font_scale: FontScale,
    pub theme: ReaderTheme,
    pub open: bool,
    pub error: Option<String>,
    /// La posición cambió desde la última persistencia.
    position_dirty: bool,
}

impl ReaderView {
    pub fn new() -> Self {
        Self {
            document: None,
            book_id: None,
            position: EpubDocument::start_position(),
            font_scale: FontScale::Normal,
            theme: ReaderTheme::Light,
            open: false,
            error: None,
            position_dirty: false,
        }
    }

    /// Abre un EPUB para el libro indicado. Si hay posición guardada se
    /// recupera ajustada a los límites reales del archivo (puede haber
    /// cambiado desde la última sesión); si no, se empieza por el principio.
    /// En caso de error se registra tal cual y no se abre nada.
    pub fn open_path(&mut self, path: &Path, book_id: u64, saved: Option<ReadingPosition>) {
        match EpubDocument::from_path(path) {
            Ok(document) => {
                self.position = match saved {
                    Some(saved) => document.clamp(saved),
                    None => EpubDocument::start_position(),
                };
                self.document = Some(document);
                self.book_id = Some(book_id);
                self.error = None;
                self.open = true;
                self.position_dirty = false;
            }
            Err(error) => {
                self.error = Some(error);
                self.open = false;
                self.document = None;
                self.book_id = None;
            }
        }
    }

    pub fn close(&mut self) {
        self.open = false;
    }

    // Los métodos de navegación siguientes son la API de estado para los
    // controles del lector (selector de capítulos, teclas); la vista actual
    // dibuja con las funciones puras homónimas para que ambas rutas no
    // diverjan. Las pruebas los ejercitan.
    #[allow(dead_code)]
    /// La posición cambió desde la última vez que la app la persistió.
    pub fn position_dirty(&self) -> bool {
        self.position_dirty
    }

    /// Entrega (una sola vez por cambio) la posición a persistir, con el
    /// identificador estable del libro.
    pub fn take_position(&mut self) -> Option<(u64, ReadingPosition)> {
        if !self.position_dirty {
            return None;
        }
        self.position_dirty = false;
        match (self.book_id, self.document.is_some()) {
            (Some(book_id), true) => Some((book_id, self.position)),
            _ => None,
        }
    }

    #[allow(dead_code)] // API de navegación para controles futuros
    /// Ventana de párrafos de la página actual dentro del capítulo.
    pub fn page_range(&self) -> Option<std::ops::Range<usize>> {
        let document = self.document.as_ref()?;
        let chapter = document.chapters.get(self.position.chapter)?;
        Some(ventana_pagina(
            chapter.paragraphs.len(),
            self.position.paragraph,
            self.font_scale.page_size(),
        ))
    }

    #[allow(dead_code)] // API de navegación para controles futuros
    /// Páginas totales del capítulo actual (para «página p de q»).
    pub fn page_count(&self) -> Option<usize> {
        let document = self.document.as_ref()?;
        let chapter = document.chapters.get(self.position.chapter)?;
        Some(paginas_totales(
            chapter.paragraphs.len(),
            self.font_scale.page_size(),
        ))
    }

    #[allow(dead_code)] // API de navegación para controles futuros
    /// Avanza una página; al terminar el capítulo pasa al siguiente.
    /// Devuelve false si ya estaba al final del libro.
    pub fn next_page(&mut self) -> bool {
        let Some(document) = self.document.as_ref() else {
            return false;
        };
        let total = document
            .chapters
            .get(self.position.chapter)
            .map(|c| c.paragraphs.len())
            .unwrap_or(0);
        let avanzo = avanzar_pagina(
            total,
            document.chapters.len(),
            &mut self.position,
            self.font_scale.page_size(),
        );
        self.position_dirty |= avanzo;
        avanzo
    }

    #[allow(dead_code)] // API de navegación para controles futuros
    /// Retrocede una página; al inicio del capítulo pasa al anterior,
    /// situándose en su última página.
    pub fn previous_page(&mut self) -> bool {
        let Some(document) = self.document.as_ref() else {
            return false;
        };
        let retrocedo = retroceder_pagina(
            &document.chapters,
            &mut self.position,
            self.font_scale.page_size(),
        );
        self.position_dirty |= retrocedo;
        retrocedo
    }

    #[allow(dead_code)] // API de navegación para controles futuros
    /// Salta a un capítulo concreto (selector de la interfaz).
    pub fn go_to_chapter(&mut self, chapter: usize) -> bool {
        let Some(document) = self.document.as_ref() else {
            return false;
        };
        let salto = saltar_capitulo(document.chapters.len(), &mut self.position, chapter);
        self.position_dirty |= salto;
        salto
    }

    #[allow(dead_code)] // API de navegación para controles futuros
    pub fn next_chapter(&mut self) -> bool {
        let Some(document) = self.document.as_ref() else {
            return false;
        };
        let destino = self.position.chapter + 1;
        let salto = saltar_capitulo(document.chapters.len(), &mut self.position, destino);
        self.position_dirty |= salto;
        salto
    }

    #[allow(dead_code)] // API de navegación para controles futuros
    pub fn previous_chapter(&mut self) -> bool {
        let Some(document) = self.document.as_ref() else {
            return false;
        };
        if self.position.chapter == 0 {
            return false;
        }
        let destino = self.position.chapter - 1;
        let salto = saltar_capitulo(document.chapters.len(), &mut self.position, destino);
        self.position_dirty |= salto;
        salto
    }

    /// Dibuja la ventana del lector. Los colores del tema son locales: no se
    /// tocan los visuales globales de la aplicación.
    pub fn show(&mut self, ctx: &egui::Context) {
        if !self.open {
            return;
        }
        // Local para no prestar self.open durante todo el builder (el
        // closure necesita campos de self para la navegación).
        let mut open = self.open;
        egui::Window::new("Lector")
            .open(&mut open)
            .default_width(900.0)
            .default_height(700.0)
            .collapsible(false)
            .show(ctx, |ui| {
                // Campos disjuntos: el préstamo inmutable del documento no
                // choca con la mutación de posición/escala/tema de los
                // botones.
                let Self {
                    document,
                    position,
                    font_scale,
                    theme,
                    position_dirty,
                    ..
                } = self;
                let Some(document) = document.as_ref() else {
                    return;
                };
                Self::draw_header(ui, document, position, position_dirty, font_scale, theme);
                ui.separator();
                Self::draw_page(ui, document, position, *font_scale, *theme);
                ui.separator();
                Self::draw_footer(ui, document, position, *font_scale, *theme, position_dirty);
            });
        self.open = open;
    }

    fn draw_header(
        ui: &mut egui::Ui,
        document: &EpubDocument,
        position: &mut ReadingPosition,
        position_dirty: &mut bool,
        font_scale: &mut FontScale,
        theme: &mut ReaderTheme,
    ) {
        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                let titulo = if document.title.is_empty() {
                    "Libro sin título incrustado"
                } else {
                    &document.title
                };
                ui.strong(egui::RichText::new(titulo).color(theme.text()));
                let capitulo = document
                    .chapters
                    .get(position.chapter)
                    .and_then(|c| c.title.clone())
                    .unwrap_or_else(|| format!("Capítulo {}", position.chapter + 1));
                ui.weak(
                    egui::RichText::new(format!(
                        "Capítulo {} de {}: {}",
                        position.chapter + 1,
                        document.chapters.len(),
                        capitulo
                    ))
                    .color(theme.muted()),
                );
            });
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.small_button("Tamaño de letra").clicked() {
                    *font_scale = font_scale.cycle();
                }
                ui.label(egui::RichText::new(font_scale.label()).color(theme.muted()));
                if ui.small_button("Tema").clicked() {
                    *theme = theme.toggle();
                }
                ui.label(egui::RichText::new(theme.label()).color(theme.muted()));
            });
        });
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            if ui.button("◀ Capítulo anterior").clicked()
                && position.chapter > 0
                && saltar_capitulo(document.chapters.len(), position, position.chapter - 1)
            {
                *position_dirty = true;
            }
            if ui.button("Capítulo siguiente ▶").clicked()
                && saltar_capitulo(document.chapters.len(), position, position.chapter + 1)
            {
                *position_dirty = true;
            }
        });
    }

    fn draw_page(
        ui: &mut egui::Ui,
        document: &EpubDocument,
        position: &ReadingPosition,
        font_scale: FontScale,
        theme: ReaderTheme,
    ) {
        let Some(chapter) = document.chapters.get(position.chapter) else {
            return;
        };
        let rango = ventana_pagina(
            chapter.paragraphs.len(),
            position.paragraph,
            font_scale.page_size(),
        );
        let factor = font_scale.factor();
        egui::Frame::new()
            .fill(theme.background())
            .inner_margin(egui::Margin::symmetric(16, 12))
            .show(ui, |ui| {
                egui::ScrollArea::vertical()
                    .auto_shrink(false)
                    .show(ui, |ui| {
                        for parrafo in chapter.paragraphs[rango.clone()].iter() {
                            ui.add_space(6.0);
                            ui.label(
                                egui::RichText::new(parrafo)
                                    .color(theme.text())
                                    .size(16.0 * factor),
                            );
                        }
                        if chapter.paragraphs.is_empty() {
                            ui.weak(
                                egui::RichText::new("(capítulo sin texto)").color(theme.muted()),
                            );
                        }
                    });
            });
    }

    fn draw_footer(
        ui: &mut egui::Ui,
        document: &EpubDocument,
        position: &mut ReadingPosition,
        font_scale: FontScale,
        theme: ReaderTheme,
        position_dirty: &mut bool,
    ) {
        let size = font_scale.page_size();
        let Some(chapter) = document.chapters.get(position.chapter) else {
            return;
        };
        let total = chapter.paragraphs.len();
        let paginas = paginas_totales(total, size);
        let actual = position.paragraph / size + 1;
        ui.horizontal(|ui| {
            if ui.button("◀ Página anterior").clicked()
                && retroceder_pagina(&document.chapters, position, size)
            {
                *position_dirty = true;
            }
            if ui.button("Página siguiente ▶").clicked()
                && avanzar_pagina(total, document.chapters.len(), position, size)
            {
                *position_dirty = true;
            }
            ui.weak(
                egui::RichText::new(format!(
                    "Página {actual} de {paginas} · leído {}%",
                    progreso(document, position)
                ))
                .color(theme.muted()),
            );
        });
    }
}

impl Default for ReaderView {
    fn default() -> Self {
        Self::new()
    }
}

// Funciones puras compartidas por el estado, la vista y las pruebas: así la
// navegación no puede divergir entre lo probado y lo dibujado.

/// Ventana de párrafos visible para una página, dentro de los límites.
fn ventana_pagina(total: usize, paragraph: usize, size: usize) -> std::ops::Range<usize> {
    let start = paragraph.min(total);
    let end = (start + size).min(total);
    start..end
}

/// Páginas en que se divide un capítulo de `total` párrafos.
fn paginas_totales(total: usize, size: usize) -> usize {
    total.div_ceil(size).max(1)
}

/// Avanza una página; al agotarse el capítulo pasa al siguiente. Devuelve
/// false si ya estaba al final del libro.
fn avanzar_pagina(
    total: usize,
    total_capitulos: usize,
    position: &mut ReadingPosition,
    size: usize,
) -> bool {
    let nueva = position.paragraph + size;
    if nueva < total.max(1) {
        position.paragraph = nueva;
        return true;
    }
    if position.chapter + 1 < total_capitulos {
        position.chapter += 1;
        position.paragraph = 0;
        return true;
    }
    false
}

/// Retrocede una página; al inicio del capítulo pasa al anterior, situándose
/// en su última página. Devuelve false si ya estaba al inicio del libro.
fn retroceder_pagina(
    chapters: &[crate::reader_epub::Chapter],
    position: &mut ReadingPosition,
    size: usize,
) -> bool {
    if position.paragraph >= size {
        position.paragraph -= size;
        return true;
    }
    if position.chapter > 0 {
        position.chapter -= 1;
        let previa = chapters
            .get(position.chapter)
            .map(|c| c.paragraphs.len())
            .unwrap_or(0);
        position.paragraph = previa.saturating_sub(1) / size * size;
        return true;
    }
    false
}

/// Salta al capítulo `destino` si existe y es distinto del actual.
fn saltar_capitulo(total_capitulos: usize, position: &mut ReadingPosition, destino: usize) -> bool {
    if destino >= total_capitulos || destino == position.chapter {
        return false;
    }
    position.chapter = destino;
    position.paragraph = 0;
    true
}

/// Porcentaje leído sobre el total de párrafos del libro (aproximación
/// honesta: los párrafos varían de longitud, se presenta como referencia).
fn progreso(document: &EpubDocument, position: &ReadingPosition) -> usize {
    let total: usize = document.chapters.iter().map(|c| c.paragraphs.len()).sum();
    if total == 0 {
        return 0;
    }
    let previos: usize = document
        .chapters
        .iter()
        .take(position.chapter)
        .map(|c| c.paragraphs.len())
        .sum();
    ((previos + position.paragraph) * 100 / total).min(100)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reader_epub::Chapter;

    fn documento_prueba() -> EpubDocument {
        let capitulo = |titulo: &str, parrafos: usize| Chapter {
            title: if titulo.is_empty() {
                None
            } else {
                Some(titulo.to_owned())
            },
            paragraphs: (0..parrafos).map(|i| format!("Párrafo {i}")).collect(),
        };
        EpubDocument {
            title: "Obra de prueba".to_owned(),
            author: "Autor".to_owned(),
            chapters: vec![capitulo("Uno", 10), capitulo("", 5), capitulo("Tres", 30)],
        }
    }

    fn lector_abierto() -> ReaderView {
        let mut view = ReaderView::new();
        view.document = Some(documento_prueba());
        view.book_id = Some(7);
        view.open = true;
        view
    }

    #[test]
    fn paginacion_avanza_y_pasa_de_capitulo_cuando_no_cabe_otra_pagina() {
        let mut view = lector_abierto();
        // Capítulo 0 con 10 párrafos y página de 14: una sola página.
        assert_eq!(view.page_range(), Some(0..10));
        assert_eq!(view.page_count(), Some(1));
        // Siguiente página pasa al capítulo 1 (no cabe más en el 0).
        assert!(view.next_page());
        assert_eq!(
            view.position,
            ReadingPosition {
                chapter: 1,
                paragraph: 0
            }
        );
        assert_eq!(view.page_range(), Some(0..5));
        assert!(view.next_page());
        assert_eq!(
            view.position,
            ReadingPosition {
                chapter: 2,
                paragraph: 0
            }
        );
        // Capítulo de 30 párrafos con páginas de 14: 0, 14, 28 y fin.
        assert!(view.next_page());
        assert_eq!(view.position.paragraph, 14);
        assert!(view.next_page());
        assert_eq!(view.position.paragraph, 28);
        assert_eq!(view.page_range(), Some(28..30));
        assert!(!view.next_page(), "fin del libro");
    }

    #[test]
    fn pagina_anterior_vuelve_y_en_borde_de_capitulo_cae_en_ultima_pagina() {
        let mut view = lector_abierto();
        view.font_scale = FontScale::Small; // 14/0.85 = 16,47 → 16 párrafos
        assert_eq!(view.font_scale.page_size(), 16);
        assert!(view.go_to_chapter(2));
        view.position.paragraph = 16; // última página del capítulo de 30
        assert!(view.previous_page());
        assert_eq!(view.position.paragraph, 0);
        assert!(view.previous_page());
        assert_eq!(
            view.position,
            ReadingPosition {
                chapter: 1,
                paragraph: 0
            }
        );
        assert!(view.previous_page());
        assert_eq!(
            view.position,
            ReadingPosition {
                chapter: 0,
                paragraph: 0
            },
            "capítulo previo con una sola página: su última página empieza en 0"
        );
        assert!(!view.previous_page(), "ya al inicio del libro");
    }

    #[test]
    fn saltos_de_capitulo_respetan_limites_y_marcan_posicion() {
        let mut view = lector_abierto();
        assert!(!view.position_dirty());
        assert!(view.go_to_chapter(2));
        assert!(view.position_dirty());
        assert!(!view.go_to_chapter(9), "capítulo fuera de rango");
        assert_eq!(view.position.chapter, 2);
        assert!(view.previous_chapter());
        assert_eq!(view.position.chapter, 1);
        assert!(view.next_chapter());
        assert_eq!(view.position.chapter, 2);
        assert!(!view.next_chapter(), "último capítulo");
    }

    #[test]
    fn apertura_con_epub_real_recupera_posicion_guardada_ajustada() {
        let mut view = ReaderView::new();
        // Archivo inexistente: error honesto, nada abierto.
        let ruta = std::env::temp_dir().join(format!(
            "mycalibro_reader_fallo_{}.epub",
            std::process::id()
        ));
        view.open_path(&ruta, 1, None);
        assert!(view.error.is_some());
        assert!(!view.open);
        assert!(view.document.is_none());

        // EPUB real de prueba (generado por las pruebas de reader_epub):
        // posición imposible se ajusta a los límites reales del archivo.
        let ruta_epub =
            std::env::temp_dir().join(format!("mycalibro_reader_ok_{}.epub", std::process::id()));
        std::fs::write(&ruta_epub, crate::reader_epub::tests::epub_de_prueba()).unwrap();
        view.open_path(
            &ruta_epub,
            42,
            Some(ReadingPosition {
                chapter: 9,
                paragraph: 99,
            }),
        );
        assert!(view.error.is_none(), "error: {:?}", view.error);
        assert!(view.open);
        assert_eq!(view.book_id, Some(42));
        let libro = view.document.as_ref().unwrap();
        let ultimo = libro.chapters.len() - 1;
        assert_eq!(
            view.position.chapter, ultimo,
            "posición imposible ajustada al último capítulo"
        );
        assert!(view.position.paragraph < libro.chapters[ultimo].paragraphs.len().max(1));
        let _ = std::fs::remove_file(&ruta_epub);
    }

    #[test]
    fn la_posicion_se_entrega_una_vez_por_cambio_y_con_libro() {
        let mut view = lector_abierto();
        assert!(view.take_position().is_none(), "sin cambios aún");
        assert!(view.next_page());
        let entregada = view.take_position().expect("cambio pendiente");
        assert_eq!(entregada.0, 7);
        assert_eq!(
            entregada.1,
            ReadingPosition {
                chapter: 1,
                paragraph: 0
            }
        );
        assert!(view.take_position().is_none(), "una sola vez por cambio");
        // Sin documento no se entrega nada aunque hubiera marca.
        let mut vacio = ReaderView::new();
        vacio.book_id = Some(3);
        vacio.position_dirty = true;
        assert!(vacio.take_position().is_none());
    }

    #[test]
    fn escala_de_letra_y_tema_ciclan_y_ajustan_la_pagina() {
        let mut view = lector_abierto();
        assert_eq!(view.font_scale, FontScale::Normal);
        assert_eq!(view.font_scale.page_size(), 14);
        view.font_scale = view.font_scale.cycle();
        assert_eq!(view.font_scale, FontScale::Large);
        assert_eq!(view.font_scale.page_size(), 11);
        assert_eq!(view.theme, ReaderTheme::Light);
        view.theme = view.theme.toggle();
        assert_eq!(view.theme, ReaderTheme::Dark);
        assert_eq!(view.theme.toggle(), ReaderTheme::Light);
        // El rango de página se ajusta al nuevo tamaño sin perder posición.
        assert_eq!(view.page_range(), Some(0..10));
    }

    #[test]
    fn clamp_y_progreso_presentado_son_razonables() {
        let libro = documento_prueba();
        let ajustada = libro.clamp(ReadingPosition {
            chapter: 9,
            paragraph: 999,
        });
        assert_eq!(
            ajustada,
            ReadingPosition {
                chapter: 2,
                paragraph: 29
            }
        );
        let mut view = lector_abierto();
        assert_eq!(progreso(view.document.as_ref().unwrap(), &view.position), 0);
        view.position = ReadingPosition {
            chapter: 1,
            paragraph: 0,
        };
        // 10 párrafos previos de 45 totales: 22 %.
        assert_eq!(
            progreso(view.document.as_ref().unwrap(), &view.position),
            22
        );
        view.position = ReadingPosition {
            chapter: 2,
            paragraph: 29,
        };
        // En la última posición válida el lector está EN el último párrafo,
        // no pasado: 44 de 45 párrafos = 97 % (aproximación honesta).
        assert_eq!(
            progreso(view.document.as_ref().unwrap(), &view.position),
            97
        );
    }
}
