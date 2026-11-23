//! Motor de escaneo y análisis de lotes para la importación masiva (PROD-010).
//!
//! Lógica pura de solo lectura: recorre archivos y carpetas seleccionados,
//! identifica candidatos con formato de libro reconocido y los contrasta con
//! el catálogo existente para proponer acciones (importar, omitir o asociar
//! formato). Nunca fusiona fichas ni escribe archivos: las decisiones las
//! toma el usuario desde el asistente de importación.
//!
//! El módulo es autónomo (solo `std`) para poder verificarse y revisarse sin
//! tocar `app/src/main.rs`. La integración se limitará a un `mod import_scan;`
//! y a convertir `Book` en `CatalogItem` desde la interfaz.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

/// Extensiones reconocidas como libro. Mantiene la misma lista que
/// `SUPPORTED_BOOK_EXTENSIONS` en main.rs; al integrarse, main.rs debería
/// reutilizar esta constante para no duplicar la fuente.
pub const SUPPORTED_BOOK_EXTENSIONS: &[&str] = &[
    "epub", "mobi", "azw", "azw3", "azw4", "kfx", "prc", "pdf", "txt", "html", "htm", "xhtml",
    "rtf", "fb2", "fbz", "doc", "docx", "odt", "lrf", "pdb", "pml", "rb", "snb", "tcr", "chm",
    "djvu", "djv", "cbz", "cbr", "cb7", "cbc", "zip", "rar",
];

/// Umbral de similitud de título/autor (coeficiente de Dice sobre bigramas
/// tras normalizar) a partir del cual se avisa de posible coincidencia.
/// Es un aviso de baja certeza: nunca autoriza fusionar automáticamente.
const SIMILARITY_THRESHOLD: f32 = 0.85;

/// Entrada mínima del catálogo existente para el contraste de duplicados.
/// En la integración se construirá desde `Book` de main.rs.
#[derive(Clone, Debug)]
pub struct CatalogItem {
    pub id: u64,
    pub title: String,
    pub author: String,
    pub path: Option<String>,
}

impl CatalogItem {
    pub fn new(
        id: u64,
        title: impl Into<String>,
        author: impl Into<String>,
        path: Option<String>,
    ) -> Self {
        Self {
            id,
            title: title.into(),
            author: author.into(),
            path,
        }
    }
}

/// Archivo candidato detectado en el escaneo.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Candidate {
    pub path: PathBuf,
    pub size_bytes: u64,
}

/// Error puntual durante el escaneo; el lote continúa con el resto.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScanError {
    pub path: PathBuf,
    pub message: String,
}

/// Resultado completo del escaneo de un lote.
#[derive(Clone, Debug, Default)]
pub struct ScanOutcome {
    /// Archivos vistos en las rutas recorridas (con o sin formato soportado).
    pub files_seen: usize,
    pub errors: Vec<ScanError>,
    /// Revisión de cada candidato, ordenada por ruta.
    pub reviews: Vec<CandidateReview>,
}

/// Revisión de un candidato frente al catálogo.
#[derive(Clone, Debug)]
pub struct CandidateReview {
    pub candidate: Candidate,
    pub matches: Vec<DuplicateMatch>,
    pub suggested_action: SuggestedAction,
}

/// Coincidencia contra una ficha del catálogo.
#[derive(Clone, Debug)]
pub struct DuplicateMatch {
    pub catalog_id: u64,
    pub title: String,
    pub author: String,
    pub path: Option<String>,
    pub kind: MatchKind,
    pub certainty: Certainty,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MatchKind {
    /// La ruta del candidato coincide exactamente con la registrada.
    ExactPath,
    /// Mismo nombre base de archivo (con extensión igual o distinta) en otra carpeta.
    SameFileName,
    /// Título y autor bibliográficos parecidos; no basta para fusionar (PROD-010).
    TitleAuthorSimilar,
}

/// Grado de certeza explícito que el asistente debe mostrar al usuario.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Certainty {
    Low,
    Medium,
    High,
}

/// Acción propuesta para el candidato; el usuario decide en el asistente.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SuggestedAction {
    Import,
    Skip,
    AssociateFormat,
}

/// Escanea las rutas de entrada (archivos o carpetas, recursivo) y contrasta
/// los candidatos con el catálogo. Solo lectura; los errores puntuales se
/// registran y no abortan el lote.
pub fn scan_paths(inputs: &[PathBuf], catalog: &[CatalogItem]) -> ScanOutcome {
    scan_paths_with_progress(inputs, catalog, &mut |_, _| true)
}

/// Igual que [`scan_paths`], notificando progreso real durante el recorrido:
/// `(archivos_vistos, candidatos_encontrados)`. El callback devuelve si el
/// escaneo debe continuar; al devolver false se detiene y devuelve lo
/// acumulado hasta ese momento. Pensado para llamarse desde un hilo
/// trabajador con cancelación cooperativa y sin bloquear la interfaz
/// (PROD-010).
pub fn scan_paths_with_progress(
    inputs: &[PathBuf],
    catalog: &[CatalogItem],
    progress: &mut dyn FnMut(usize, usize) -> bool,
) -> ScanOutcome {
    let mut outcome = ScanOutcome::default();
    let mut seen: BTreeSet<String> = BTreeSet::new();
    for input in inputs {
        if !collect_candidates(input, &mut outcome, &mut seen, progress) {
            return outcome; // cancelado: se entrega lo acumulado
        }
    }
    outcome
        .reviews
        .sort_by(|a, b| a.candidate.path.cmp(&b.candidate.path));
    let refs = catalog_refs(catalog);
    for review in &mut outcome.reviews {
        review.matches = analyze_candidate(&review.candidate.path, &refs);
        review.suggested_action = suggested_action(&review.candidate.path, &review.matches);
    }
    outcome
}

/// Analiza un archivo aislado frente al catálogo (p. ej. un arrastre posterior
/// al escaneo del lote).
pub fn review_candidate(path: &Path, catalog: &[CatalogItem]) -> CandidateReview {
    let candidate = Candidate {
        path: path.to_path_buf(),
        size_bytes: std::fs::metadata(path).map(|meta| meta.len()).unwrap_or(0),
    };
    let refs = catalog_refs(catalog);
    let matches = analyze_candidate(path, &refs);
    let suggested_action = suggested_action(path, &matches);
    CandidateReview {
        candidate,
        matches,
        suggested_action,
    }
}

pub fn is_supported_book(path: &Path) -> bool {
    path.extension()
        .map(|ext| ext.to_string_lossy().to_lowercase())
        .map(|ext| SUPPORTED_BOOK_EXTENSIONS.contains(&ext.as_str()))
        .unwrap_or(false)
}

/// Clave de comparación de rutas para Windows (PROD-007): ruta canónica
/// cuando el archivo existe —resuelve mayúsculas reales, segmentos «.» y
/// «..», y enlaces— y minúsculas como normalización general. Comparar en
/// minúsculas es seguro en este producto porque NTFS no distingue mayúsculas.
fn path_key(path: &Path) -> String {
    match std::fs::canonicalize(path) {
        Ok(canonical) => canonical.to_string_lossy().to_lowercase(),
        Err(_) => path.to_string_lossy().to_lowercase(),
    }
}

/// Elemento de catálogo con su clave de ruta calculada una sola vez por
/// análisis, para no canonicalizar repetidamente en lotes grandes.
struct CatalogRef<'a> {
    item: &'a CatalogItem,
    key: Option<String>,
}

fn catalog_refs(catalog: &[CatalogItem]) -> Vec<CatalogRef<'_>> {
    catalog
        .iter()
        .map(|item| CatalogRef {
            key: item.path.as_deref().map(|path| path_key(Path::new(path))),
            item,
        })
        .collect()
}

/// Recorre recursivamente una ruta acumulando candidatos de solo lectura.
/// No sigue enlaces simbólicos de directorio (evita ciclos) y no repite
/// archivos ya vistos en el mismo lote, aunque se alcancen por rutas
/// equivalentes (relativas, mayúsculas distintas o segmentos «..»).
/// Notifica progreso tras cada archivo procesado; devolver false desde el
/// callback detiene el recorrido de forma cooperativa.
fn collect_candidates(
    input: &Path,
    outcome: &mut ScanOutcome,
    seen: &mut BTreeSet<String>,
    progress: &mut dyn FnMut(usize, usize) -> bool,
) -> bool {
    let key = path_key(input);
    if !seen.insert(key) {
        return true;
    }
    let meta = match std::fs::symlink_metadata(input) {
        Ok(meta) => meta,
        Err(error) => {
            outcome.errors.push(ScanError {
                path: input.to_path_buf(),
                message: error.to_string(),
            });
            return true;
        }
    };
    if meta.is_file() {
        outcome.files_seen += 1;
        if is_supported_book(input) {
            let size_bytes = std::fs::metadata(input)
                .map(|m| m.len())
                .unwrap_or(meta.len());
            outcome.reviews.push(CandidateReview {
                candidate: Candidate {
                    path: input.to_path_buf(),
                    size_bytes,
                },
                matches: Vec::new(),
                suggested_action: SuggestedAction::Import,
            });
        }
        return progress(outcome.files_seen, outcome.reviews.len());
    }
    if meta.is_dir() {
        let entries = match std::fs::read_dir(input) {
            Ok(entries) => entries,
            Err(error) => {
                outcome.errors.push(ScanError {
                    path: input.to_path_buf(),
                    message: error.to_string(),
                });
                return true;
            }
        };
        for entry in entries {
            match entry {
                Ok(entry) => {
                    if !collect_candidates(&entry.path(), outcome, seen, progress) {
                        return false;
                    }
                }
                Err(error) => outcome.errors.push(ScanError {
                    path: input.to_path_buf(),
                    message: error.to_string(),
                }),
            }
        }
    }
    true
}

/// Contrasta un candidato con el catálogo y devuelve todas las coincidencias
/// encontradas, cada una con su grado de certeza.
fn analyze_candidate(path: &Path, catalog: &[CatalogRef]) -> Vec<DuplicateMatch> {
    let mut matches = Vec::new();
    let candidate_key = path_key(path);
    let stem = candidate_stem(path);
    let candidate_stem_lower = stem.to_lowercase();
    // En el patrón «Título - Autor», la parte del título va antes del guion.
    let (candidate_title, candidate_author) = match stem.split_once(" - ") {
        Some((title, author)) => (title.trim().to_owned(), author.trim().to_owned()),
        None => (stem.clone(), String::new()),
    };
    for r in catalog {
        let item = r.item;
        if let Some(existing) = item.path.as_deref() {
            // La igualdad de rutas es canónica e insensible a mayúsculas
            // (path_key), no una comparación literal de cadenas.
            if r.key.as_deref() == Some(candidate_key.as_str()) {
                matches.push(DuplicateMatch {
                    catalog_id: item.id,
                    title: item.title.clone(),
                    author: item.author.clone(),
                    path: Some(existing.to_owned()),
                    kind: MatchKind::ExactPath,
                    certainty: Certainty::High,
                });
                continue;
            }
            let existing_stem = Path::new(existing)
                .file_stem()
                .map(|stem| stem.to_string_lossy().to_lowercase())
                .unwrap_or_default();
            if !candidate_stem_lower.is_empty() && candidate_stem_lower == existing_stem {
                matches.push(DuplicateMatch {
                    catalog_id: item.id,
                    title: item.title.clone(),
                    author: item.author.clone(),
                    path: Some(existing.to_owned()),
                    kind: MatchKind::SameFileName,
                    certainty: Certainty::Medium,
                });
                continue;
            }
        }
        if titles_similar(&item.title, &candidate_title)
            && authors_similar(&item.author, &candidate_author)
        {
            matches.push(DuplicateMatch {
                catalog_id: item.id,
                title: item.title.clone(),
                author: item.author.clone(),
                path: item.path.clone(),
                kind: MatchKind::TitleAuthorSimilar,
                certainty: Certainty::Low,
            });
        }
    }
    matches
}

/// Acción sugerida a partir de la coincidencia más fuerte. Las sugerencias
/// nunca ejecutan fusiones ni borrados: solo orientan la decisión del usuario.
fn suggested_action(candidate_path: &Path, matches: &[DuplicateMatch]) -> SuggestedAction {
    let strongest = matches.iter().max_by_key(|m| m.certainty);
    match strongest {
        Some(DuplicateMatch {
            kind: MatchKind::ExactPath,
            ..
        }) => SuggestedAction::Skip,
        Some(DuplicateMatch {
            kind: MatchKind::SameFileName,
            path,
            ..
        }) => {
            // Mismo nombre de archivo con otra extensión suele indicar otro
            // formato de la misma obra; con la misma extensión, probable
            // duplicado idéntico.
            let extension_of = |p: &Path| {
                p.extension()
                    .map(|ext| ext.to_string_lossy().to_lowercase())
                    .unwrap_or_default()
            };
            let existing_ext = path
                .as_deref()
                .map(Path::new)
                .map(extension_of)
                .unwrap_or_default();
            if !existing_ext.is_empty() && existing_ext == extension_of(candidate_path) {
                SuggestedAction::Skip
            } else {
                SuggestedAction::AssociateFormat
            }
        }
        Some(_) | None => SuggestedAction::Import,
    }
}

fn titles_similar(a: &str, b: &str) -> bool {
    let (na, nb) = (normalize(a), normalize(b));
    if na.is_empty() || nb.is_empty() {
        return false;
    }
    if na == nb || na.contains(&nb) || nb.contains(&na) {
        return true;
    }
    dice_similarity(&na, &nb) >= SIMILARITY_THRESHOLD
}

fn authors_similar(a: &str, b: &str) -> bool {
    let (na, nb) = (normalize(a), normalize(b));
    if na.is_empty() && nb.is_empty() {
        // Sin autor en ambos lados no se puede confirmar nada; solo se avisa
        // si el título coincide, que ya se comprueba antes de llamar aquí.
        return true;
    }
    if na.is_empty() || nb.is_empty() {
        return false;
    }
    if na == nb || na.contains(&nb) || nb.contains(&na) {
        return true;
    }
    dice_similarity(&na, &nb) >= SIMILARITY_THRESHOLD
}

/// Normaliza para comparar: minúsculas, solo alfanuméricos y espacios plegados.
fn normalize(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut last_space = false;
    for ch in text.to_lowercase().chars() {
        if ch.is_alphanumeric() {
            out.push(ch);
            last_space = false;
        } else if !last_space && !out.is_empty() {
            out.push(' ');
            last_space = true;
        }
    }
    out.trim().to_owned()
}

/// Similitud de Dice sobre bigramas de caracteres (1.0 = idénticos).
fn dice_similarity(a: &str, b: &str) -> f32 {
    let bigrams = |text: &str| -> BTreeSet<(char, char)> {
        let chars: Vec<char> = text.chars().filter(|c| !c.is_whitespace()).collect();
        (0..chars.len().saturating_sub(1))
            .map(|i| (chars[i], chars[i + 1]))
            .collect()
    };
    let (ba, bb) = (bigrams(a), bigrams(b));
    if ba.is_empty() || bb.is_empty() {
        return 0.0;
    }
    let common = ba.intersection(&bb).count();
    2.0 * common as f32 / (ba.len() + bb.len()) as f32
}

fn candidate_stem(path: &Path) -> String {
    path.file_stem()
        .map(|stem| stem.to_string_lossy().to_string())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn catalog_sample() -> Vec<CatalogItem> {
        vec![
            CatalogItem::new(
                1,
                "1984",
                "George Orwell",
                Some("C:\\biblio\\1984.epub".to_owned()),
            ),
            CatalogItem::new(2, "El nombre del viento", "Patrick Rothfuss", None),
        ]
    }

    #[test]
    fn reconoce_extensiones_soportadas_y_descarta_las_demas() {
        assert!(is_supported_book(Path::new("libro.EPUB")));
        assert!(is_supported_book(Path::new("libro.Azw3")));
        assert!(is_supported_book(Path::new("/x/ruta/libro.pdf")));
        assert!(!is_supported_book(Path::new("imagen.png")));
        assert!(!is_supported_book(Path::new("sinextension")));
    }

    #[test]
    fn coincidencia_ruta_exacta_es_certeza_alta_y_sugiere_omitir() {
        let catalog = catalog_sample();
        let review = review_candidate(Path::new("C:\\biblio\\1984.epub"), &catalog);
        assert_eq!(review.matches.len(), 1);
        assert_eq!(review.matches[0].kind, MatchKind::ExactPath);
        assert_eq!(review.matches[0].certainty, Certainty::High);
        assert_eq!(review.suggested_action, SuggestedAction::Skip);
    }

    #[test]
    fn coincidencia_ruta_exacta_ignora_mayusculas_y_segmentos_redundantes() {
        // Archivo real en disco: la clave canónica debe igualar rutas que
        // difieren solo en mayúsculas o en segmentos «.» intermedios.
        let base = std::env::temp_dir().join(format!("mycalibro_clave_{}", std::process::id()));
        std::fs::create_dir_all(&base).unwrap();
        let real = base.join("1984.epub");
        std::fs::write(&real, b"x").unwrap();
        let catalog = vec![CatalogItem::new(
            1,
            "1984",
            "George Orwell",
            Some(real.display().to_string().to_uppercase()),
        )];
        let review = review_candidate(&base.join(".").join("1984.epub"), &catalog);
        assert_eq!(review.matches.len(), 1);
        assert_eq!(review.matches[0].kind, MatchKind::ExactPath);
        assert_eq!(review.suggested_action, SuggestedAction::Skip);
        std::fs::remove_dir_all(&base).unwrap();
    }

    #[test]
    fn rutas_equivalentes_no_duplican_candidatos_en_el_lote() {
        let base = std::env::temp_dir().join(format!("mycalibro_equiv_{}", std::process::id()));
        std::fs::create_dir_all(base.join("sub")).unwrap();
        std::fs::write(base.join("sub").join("libro.epub"), b"x").unwrap();
        // La misma carpeta se pasa por tres rutas distintas: directa, con «.»
        // y con «sub/..» de vuelta; el escaneo debe deduplicarlas.
        let redundante = base.join("sub").join("..").join("sub");
        let outcome = scan_paths(&[base.clone(), base.join(".").join("sub"), redundante], &[]);
        assert_eq!(
            outcome.reviews.len(),
            1,
            "un solo candidato aunque haya tres rutas equivalentes"
        );
        assert_eq!(outcome.errors.len(), 0);
        std::fs::remove_dir_all(&base).unwrap();
    }

    #[test]
    fn el_progreso_se_notifica_incrementalmente_y_coincide_con_el_resultado() {
        let base = std::env::temp_dir().join(format!("mycalibro_prog_{}", std::process::id()));
        std::fs::create_dir_all(&base).unwrap();
        std::fs::write(base.join("a.epub"), b"x").unwrap();
        std::fs::write(base.join("b.pdf"), b"y").unwrap();
        std::fs::write(base.join("c.txt"), b"z").unwrap();
        std::fs::write(base.join("d.png"), b"w").unwrap();
        let mut eventos: Vec<(usize, usize)> = Vec::new();
        let outcome = scan_paths_with_progress(&[base.clone()], &[], &mut |files, candidates| {
            eventos.push((files, candidates));
            true
        });
        assert!(
            !eventos.is_empty(),
            "debe haber al menos un evento de progreso"
        );
        let (ultimo_files, ultimo_candidatos) = *eventos.last().unwrap();
        assert_eq!(ultimo_files, outcome.files_seen);
        assert_eq!(ultimo_candidatos, outcome.reviews.len());
        // Monótono no decreciente en ambas componentes.
        assert!(eventos
            .windows(2)
            .all(|ventana| ventana[0].0 <= ventana[1].0 && ventana[0].1 <= ventana[1].1));
        std::fs::remove_dir_all(&base).unwrap();
    }

    #[test]
    fn el_escaneo_se_detiene_de_forma_cooperativa_al_devolver_false() {
        let base = std::env::temp_dir().join(format!("mycalibro_cancel_{}", std::process::id()));
        std::fs::create_dir_all(&base).unwrap();
        for nombre in ["a.epub", "b.epub", "c.epub", "d.epub", "e.epub", "f.epub"] {
            std::fs::write(base.join(nombre), b"x").unwrap();
        }
        let mut llamadas = 0;
        let outcome = scan_paths_with_progress(&[base.clone()], &[], &mut |files, _| {
            llamadas += 1;
            files < 2 // cancelar después del segundo archivo
        });
        assert!(
            llamadas < 6,
            "el recorrido se detiene antes de terminar: {llamadas} llamadas"
        );
        assert!(
            outcome.files_seen < 6,
            "no se llegaron a ver todos los archivos"
        );
        assert!(outcome.errors.is_empty());
        std::fs::remove_dir_all(&base).unwrap();
    }

    #[test]
    fn mismo_nombre_en_otra_carpeta_es_certeza_media() {
        let catalog = catalog_sample();
        let review = review_candidate(Path::new("C:\\descargas\\1984.epub"), &catalog);
        assert_eq!(review.matches.len(), 1);
        assert_eq!(review.matches[0].kind, MatchKind::SameFileName);
        assert_eq!(review.matches[0].certainty, Certainty::Medium);
        // Misma extensión: probable duplicado idéntico.
        assert_eq!(review.suggested_action, SuggestedAction::Skip);
    }

    #[test]
    fn mismo_nombre_con_otra_extension_sugiere_asociar_formato() {
        let catalog = vec![CatalogItem::new(
            1,
            "1984",
            "George Orwell",
            Some("C:\\biblio\\1984.epub".to_owned()),
        )];
        let review = review_candidate(Path::new("C:\\descargas\\1984.mobi"), &catalog);
        assert_eq!(review.matches[0].kind, MatchKind::SameFileName);
        assert_eq!(review.suggested_action, SuggestedAction::AssociateFormat);
    }

    #[test]
    fn titulo_y_autor_parecidos_son_certeza_baja_y_sugieren_importar() {
        let catalog = vec![CatalogItem::new(
            3,
            "El nombre del viento (edición 10º aniversario)",
            "Patrick Rothfuss",
            None,
        )];
        let review = review_candidate(
            Path::new("C:\\x\\El Nombre del Viento - Patrick Rothfuss.epub"),
            &catalog,
        );
        assert_eq!(review.matches.len(), 1);
        assert_eq!(review.matches[0].kind, MatchKind::TitleAuthorSimilar);
        assert_eq!(review.matches[0].certainty, Certainty::Low);
        // Nunca se propone fusionar automáticamente por título/autor.
        assert_eq!(review.suggested_action, SuggestedAction::Import);
    }

    #[test]
    fn titulo_igual_con_autor_distinto_no_coincide() {
        let catalog = vec![CatalogItem::new(4, "1984", "Autor Distinto", None)];
        let review = review_candidate(Path::new("C:\\x\\1984 - Otra Persona.epub"), &catalog);
        assert!(review.matches.is_empty());
        assert_eq!(review.suggested_action, SuggestedAction::Import);
    }

    #[test]
    fn catalogo_vacio_importa_sin_coincidencias() {
        let review = review_candidate(Path::new("C:\\x\\libro nuevo.epub"), &[]);
        assert!(review.matches.is_empty());
        assert_eq!(review.suggested_action, SuggestedAction::Import);
    }

    #[test]
    fn escaneo_recursivo_encuentra_solo_soportados_y_deduplica() {
        let base = std::env::temp_dir().join(format!("mycalibro_scan_{}", std::process::id()));
        let dir_a = base.join("autores");
        let dir_b = base.join("autores").join("orwell");
        std::fs::create_dir_all(&dir_b).unwrap();
        std::fs::write(dir_a.join("1984.epub"), b"x").unwrap();
        std::fs::write(dir_b.join("ensayo.pdf"), b"yy").unwrap();
        std::fs::write(base.join("portada.png"), b"z").unwrap();

        // Rutas solapadas (base y subcarpeta) no deben producir duplicados.
        let outcome = scan_paths(&[base.clone(), dir_b.clone()], &[]);
        assert_eq!(
            outcome.reviews.len(),
            2,
            "solo epub y pdf; png descartado y sin duplicados"
        );
        assert_eq!(outcome.files_seen, 3);
        assert_eq!(outcome.errors.len(), 0);

        std::fs::remove_dir_all(&base).unwrap();
    }

    #[test]
    fn ruta_inexistente_se_registra_como_error_sin_abortar() {
        let outcome = scan_paths(&[Path::new("Z:\\no\\existe").to_path_buf()], &[]);
        assert_eq!(outcome.reviews.len(), 0);
        assert_eq!(outcome.errors.len(), 1);
    }

    #[test]
    fn escaneo_de_lote_contrasta_con_el_catalogo() {
        let base = std::env::temp_dir().join(format!("mycalibro_lote_{}", std::process::id()));
        std::fs::create_dir_all(&base).unwrap();
        std::fs::write(base.join("1984.epub"), b"x").unwrap();
        std::fs::write(base.join("nuevo.epub"), b"y").unwrap();

        let outcome = scan_paths(&[base.clone()], &catalog_sample());
        assert_eq!(outcome.reviews.len(), 2);
        let repetido = outcome
            .reviews
            .iter()
            .find(|r| r.candidate.path.file_name().unwrap() == "1984.epub")
            .unwrap();
        assert_eq!(repetido.suggested_action, SuggestedAction::Skip);
        let nuevo = outcome
            .reviews
            .iter()
            .find(|r| r.candidate.path.file_name().unwrap() == "nuevo.epub")
            .unwrap();
        assert!(nuevo.matches.is_empty());
        assert_eq!(nuevo.suggested_action, SuggestedAction::Import);

        std::fs::remove_dir_all(&base).unwrap();
    }

    #[test]
    fn normalizacion_y_similitud_son_robustas() {
        assert_eq!(
            normalize("  El  Nombre-del Viento! "),
            "el nombre del viento"
        );
        assert!(titles_similar("1984", "1984"));
        assert!(titles_similar("1984", "1984 - George Orwell"));
        assert!(!titles_similar("1984", "Fahrenheit 451"));
        assert!(authors_similar("George Orwell", "george orwell"));
        assert!(authors_similar("", ""));
        assert!(!authors_similar("Orwell", ""));
    }
}
