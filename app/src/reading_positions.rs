//! Posiciones de lectura persistentes por libro (PROD-004, PROD-008).
//!
//! Guarda la última posición de cada libro —capítulo/párrafo para EPUB,
//! página para PDF— en `reading-positions.json` dentro de la carpeta de
//! biblioteca, con escritura atómica de la capa de persistencia. La clave es
//! el identificador estable del libro, así que las posiciones sobreviven a
//! cambios de ruta y vuelven a encajar tras una restauración de copia.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
#[cfg(test)]
use std::path::{Path, PathBuf};

use crate::reader_epub::ReadingPosition;

/// Nombre del archivo dentro de la carpeta de biblioteca.
pub const FILE_NAME: &str = "reading-positions.json";

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct StoredPosition {
    /// EPUB: capítulo actual.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub chapter: Option<usize>,
    /// EPUB: párrafo dentro del capítulo (estable ante cambios de tipografía).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub paragraph: Option<usize>,
    /// PDF: página actual.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub page: Option<u32>,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ReaderPreferences {
    pub font_size: u8,
    pub dark: bool,
    pub pdf_zoom: f32,
    pub pdf_dark: bool,
}
impl Default for ReaderPreferences {
    fn default() -> Self {
        Self {
            font_size: 1,
            dark: false,
            pdf_zoom: 1.0,
            pdf_dark: false,
        }
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ReadingPositions {
    #[serde(default)]
    pub preferences: ReaderPreferences,
    #[serde(default)]
    positions: BTreeMap<u64, StoredPosition>,
}

/// Carga el archivo de la carpeta indicada. Un archivo ausente no es error;
/// uno corrupto se informa y se empieza vacío sin destruirlo (solo una
/// posición nueva lo reescribe).
#[cfg(test)]
pub fn load(folder: &Path) -> (ReadingPositions, Option<String>) {
    let ruta = folder.join(FILE_NAME);
    match std::fs::read(&ruta) {
        Ok(bytes) => match serde_json::from_slice::<ReadingPositions>(&bytes) {
            Ok(positions) => (positions, None),
            Err(error) => (
                ReadingPositions::default(),
                Some(format!(
                    "Posiciones de lectura ilegibles, se empiezan de cero: {error}"
                )),
            ),
        },
        Err(_) => (ReadingPositions::default(), None),
    }
}

impl ReadingPositions {
    #[cfg(test)]
    pub fn save(&self, folder: &Path) -> Result<(), String> {
        let ruta: PathBuf = folder.join(FILE_NAME);
        let bytes = serde_json::to_vec_pretty(self).map_err(|e| e.to_string())?;
        crate::persistence::atomic_write(&ruta, &bytes)
    }

    /// Posición EPUB guardada del libro, si existe.
    pub fn epub_position(&self, book_id: u64) -> Option<ReadingPosition> {
        let guardada = self.positions.get(&book_id)?;
        Some(ReadingPosition {
            chapter: guardada.chapter.unwrap_or(0),
            paragraph: guardada.paragraph.unwrap_or(0),
        })
    }

    /// Página PDF guardada del libro, si existe.
    pub fn pdf_page(&self, book_id: u64) -> Option<u32> {
        self.positions.get(&book_id)?.page
    }

    /// Guarda la posición EPUB del libro.
    pub fn set_epub(&mut self, book_id: u64, position: ReadingPosition) {
        let entry = self.positions.entry(book_id).or_default();
        entry.chapter = Some(position.chapter);
        entry.paragraph = Some(position.paragraph);
    }

    /// Guarda la página PDF del libro.
    pub fn set_pdf(&mut self, book_id: u64, page: u32) {
        let entry = self.positions.entry(book_id).or_default();
        entry.page = Some(page);
    }

    /// Al eliminar un libro del catálogo se elimina también su posición.
    pub fn forget(&mut self, book_id: u64) {
        self.positions.remove(&book_id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn old_positions_get_default_preferences_and_new_preferences_roundtrip() {
        let mut positions: ReadingPositions = serde_json::from_str(r#"{"positions":{}}"#).unwrap();
        assert_eq!(positions.preferences, ReaderPreferences::default());
        positions.preferences = ReaderPreferences {
            font_size: 3,
            dark: true,
            pdf_zoom: 1.5,
            pdf_dark: true,
        };
        let restored: ReadingPositions =
            serde_json::from_slice(&serde_json::to_vec(&positions).unwrap()).unwrap();
        assert_eq!(restored.preferences, positions.preferences);
    }
    #[test]
    fn ausente_no_es_error_y_corrupto_se_informa_sin_destruir() {
        let base =
            std::env::temp_dir().join(format!("mycalibro_pos_ausente_{}", std::process::id()));
        let (positions, error) = load(&base);
        assert!(error.is_none());
        assert!(positions.epub_position(1).is_none());
        assert!(positions.pdf_page(1).is_none());

        // Corrupto: aviso, estado vacío, archivo original intacto hasta que
        // una posición nueva lo reescriba.
        std::fs::create_dir_all(&base).unwrap();
        std::fs::write(base.join(FILE_NAME), b"{ no soy json").unwrap();
        let (positions, error) = load(&base);
        assert!(error.is_some());
        assert!(positions.epub_position(1).is_none());
        assert_eq!(
            std::fs::read(base.join(FILE_NAME)).unwrap(),
            b"{ no soy json"
        );
        std::fs::remove_dir_all(&base).unwrap();
    }

    #[test]
    fn guardar_y_cargar_ronda_epub_y_pdf_por_identificador() {
        let base = std::env::temp_dir().join(format!("mycalibro_pos_ronda_{}", std::process::id()));
        std::fs::create_dir_all(&base).unwrap();
        let mut positions = ReadingPositions::default();
        positions.set_epub(
            1,
            ReadingPosition {
                chapter: 3,
                paragraph: 7,
            },
        );
        positions.set_pdf(2, 41);
        positions.save(&base).expect("guardado atómico");

        let (recuperado, error) = load(&base);
        assert!(error.is_none());
        assert_eq!(
            recuperado.epub_position(1),
            Some(ReadingPosition {
                chapter: 3,
                paragraph: 7
            })
        );
        assert_eq!(recuperado.pdf_page(2), Some(41));
        assert_eq!(recuperado.pdf_page(1), None, "un libro sin página PDF");
        std::fs::remove_dir_all(&base).unwrap();
    }

    #[test]
    fn actualizar_y_eliminar_posiciones() {
        let mut positions = ReadingPositions::default();
        positions.set_epub(
            5,
            ReadingPosition {
                chapter: 0,
                paragraph: 0,
            },
        );
        positions.set_epub(
            5,
            ReadingPosition {
                chapter: 2,
                paragraph: 9,
            },
        );
        assert_eq!(
            positions.epub_position(5),
            Some(ReadingPosition {
                chapter: 2,
                paragraph: 9
            }),
            "la última posición gana"
        );
        positions.set_pdf(5, 3);
        positions.set_pdf(5, 8);
        assert_eq!(positions.pdf_page(5), Some(8));
        positions.forget(5);
        assert!(positions.epub_position(5).is_none());
        assert!(positions.pdf_page(5).is_none());
    }
}
