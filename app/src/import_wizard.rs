//! Estado y plan del asistente de importación masiva (PROD-010).
//!
//! Parte pura, sin egui: la vista (`import_wizard_view.rs`, por integrar)
//! solo muestra y muta este estado. El plan resultante lo ejecutará el
//! backend integrado (crear fichas, asociar formatos); este módulo nunca
//! escribe archivos ni fusiona fichas por su cuenta.
//!
//! Flujo: `start_scan` → progreso incremental del hilo trabajador
//! (`record_progress`) → `finish_scan` con el resultado de
//! [`crate::import_scan::scan_paths`] → revisión de duplicados por el
//! usuario (`set_decision`) → `build_plan` para la ejecución.

use std::path::PathBuf;

use crate::import_scan::{
    Certainty, CatalogItem, DuplicateMatch, MatchKind, ScanError, ScanOutcome, SuggestedAction,
};

/// Fase actual del asistente.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    /// Sin escaneo iniciado.
    Idle,
    /// Escaneo en curso en el hilo trabajador.
    Scanning,
    /// Lote escaneado; el usuario revisa duplicados y decide.
    Reviewing,
}

/// Decisión del usuario sobre cada candidato del lote.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Decision {
    /// Crear una ficha nueva para este archivo.
    Import,
    /// No incorporar este archivo.
    Skip,
    /// Añadir el archivo como formato adicional de un libro ya registrado.
    AssociateFormat,
}

impl Decision {
    pub fn label(self) -> &'static str {
        match self {
            Decision::Import => "Importar",
            Decision::Skip => "Omitir",
            Decision::AssociateFormat => "Asociar formato",
        }
    }
}

/// Progreso comunicado desde el hilo trabajador durante el escaneo.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ScanProgress {
    pub files_seen: usize,
    pub candidates_found: usize,
}

/// Estado completo del asistente de importación.
#[derive(Debug)]
pub struct ImportWizard {
    pub inputs: Vec<PathBuf>,
    pub phase: Phase,
    pub progress: ScanProgress,
    /// Resultado del escaneo cuando la fase es `Reviewing`.
    pub outcome: Option<ScanOutcome>,
    /// Decisión por candidato, alineada con `outcome.reviews`.
    /// `None` en candidatos con coincidencias obliga a revisarlos.
    decisions: Vec<Option<Decision>>,
}

/// Entrada del plan final revisado por el usuario.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlanEntry {
    pub path: PathBuf,
    pub decision: Decision,
    /// Ficha del catálogo con la que se asociará el formato, si procede.
    pub associate_with: Option<u64>,
}

/// Plan listo para que el backend lo ejecute.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ImportPlan {
    pub entries: Vec<PlanEntry>,
    pub to_import: usize,
    pub to_skip: usize,
    pub to_associate: usize,
}

/// Resumen para la barra inferior del asistente.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Summary {
    pub candidates: usize,
    pub to_import: usize,
    pub to_skip: usize,
    pub to_associate: usize,
    pub undecided: usize,
    pub with_warnings: usize,
}

impl ImportWizard {
    pub fn new() -> Self {
        Self {
            inputs: Vec::new(),
            phase: Phase::Idle,
            progress: ScanProgress::default(),
            outcome: None,
            decisions: Vec::new(),
        }
    }

    /// Inicia un escaneo sobre las rutas indicadas. La vista debe lanzar el
    /// trabajo en un hilo aparte; este estado solo registra la fase.
    pub fn start_scan(&mut self, inputs: Vec<PathBuf>) {
        self.inputs = inputs;
        self.phase = Phase::Scanning;
        self.progress = ScanProgress::default();
        self.outcome = None;
        self.decisions.clear();
    }

    /// Progreso incremental comunicado desde el hilo trabajador.
    pub fn record_progress(&mut self, files_seen: usize, candidates_found: usize) {
        if self.phase == Phase::Scanning {
            self.progress = ScanProgress { files_seen, candidates_found };
        }
    }

    /// Incorpora el resultado final del escaneo y prepara la revisión.
    /// Valores por defecto conservadores: los candidatos sin coincidencias
    /// quedan marcados para importar; los que tienen coincidencias quedan
    /// sin decidir para forzar la revisión del usuario (PROD-010).
    pub fn finish_scan(&mut self, outcome: ScanOutcome) {
        self.decisions = outcome
            .reviews
            .iter()
            .map(|review| {
                if review.matches.is_empty() {
                    Some(Decision::Import)
                } else {
                    None
                }
            })
            .collect();
        self.outcome = Some(outcome);
        self.phase = Phase::Reviewing;
    }

    /// Fija la decisión del usuario para el candidato en la posición dada.
    /// Devuelve false si el índice no existe o el asistente no está revisando.
    pub fn set_decision(&mut self, index: usize, decision: Decision) -> bool {
        if self.phase != Phase::Reviewing || index >= self.decisions.len() {
            return false;
        }
        self.decisions[index] = Some(decision);
        true
    }

    /// Rellena las decisiones pendientes con las sugeridas por el análisis
    /// (conveniencia opcional; nunca obligatoria para el usuario).
    pub fn apply_suggestions(&mut self) {
        if let Some(outcome) = &self.outcome {
            for (index, review) in outcome.reviews.iter().enumerate() {
                if self.decisions[index].is_none() {
                    self.decisions[index] = Some(suggested_decision(review.suggested_action));
                }
            }
        }
    }

    /// Candidatos con coincidencias que el usuario aún no ha decidido.
    pub fn undecided_duplicates(&self) -> Vec<usize> {
        self.outcome
            .as_ref()
            .map(|outcome| {
                outcome
                    .reviews
                    .iter()
                    .enumerate()
                    .filter(|(index, review)| !review.matches.is_empty() && self.decisions[*index].is_none())
                    .map(|(index, _)| index)
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Resumen de decisiones para la interfaz.
    pub fn summary(&self) -> Summary {
        let mut summary = Summary::default();
        if let Some(outcome) = &self.outcome {
            summary.candidates = outcome.reviews.len();
            for (index, review) in outcome.reviews.iter().enumerate() {
                if !review.matches.is_empty() {
                    summary.with_warnings += 1;
                }
                match self.decisions[index] {
                    Some(Decision::Import) => summary.to_import += 1,
                    Some(Decision::Skip) => summary.to_skip += 1,
                    Some(Decision::AssociateFormat) => summary.to_associate += 1,
                    None => summary.undecided += 1,
                }
            }
        }
        summary
    }

    /// Errores puntuales del escaneo (rutas ilegibles, etc.).
    pub fn scan_errors(&self) -> &[ScanError] {
        self.outcome.as_ref().map(|outcome| outcome.errors.as_slice()).unwrap_or(&[])
    }

    /// Construye el plan si el usuario ha revisado todos los duplicados y hay
    /// trabajo que hacer. El mensaje de error está pensado para mostrarse tal
    /// cual en la interfaz.
    pub fn build_plan(&self) -> Result<ImportPlan, String> {
        let outcome = match &self.outcome {
            Some(outcome) => outcome,
            None => return Err("Primero selecciona archivos o carpetas para importar".to_owned()),
        };
        if self.phase != Phase::Reviewing {
            return Err("El escaneo sigue en curso".to_owned());
        }
        let undecided = self.undecided_duplicates();
        if !undecided.is_empty() {
            return Err(format!(
                "Revisa {} candidato(s) con posibles duplicados antes de continuar",
                undecided.len()
            ));
        }
        let mut plan = ImportPlan::default();
        for (index, review) in outcome.reviews.iter().enumerate() {
            let decision = self.decisions[index].expect("sin decidir tras validar duplicados");
            match decision {
                Decision::Import => {
                    plan.entries.push(PlanEntry {
                        path: review.candidate.path.clone(),
                        decision: Decision::Import,
                        associate_with: None,
                    });
                    plan.to_import += 1;
                }
                Decision::Skip => plan.to_skip += 1,
                Decision::AssociateFormat => {
                    plan.entries.push(PlanEntry {
                        path: review.candidate.path.clone(),
                        decision: Decision::AssociateFormat,
                        associate_with: strongest_match_id(&review.matches),
                    });
                    plan.to_associate += 1;
                }
            }
        }
        if plan.entries.is_empty() {
            return Err("No hay ningún archivo pendiente de importar o asociar".to_owned());
        }
        Ok(plan)
    }
}

impl Default for ImportWizard {
    fn default() -> Self {
        Self::new()
    }
}

fn suggested_decision(action: SuggestedAction) -> Decision {
    match action {
        SuggestedAction::Import => Decision::Import,
        SuggestedAction::Skip => Decision::Skip,
        SuggestedAction::AssociateFormat => Decision::AssociateFormat,
    }
}

/// Ficha del catálogo con la coincidencia más fuerte, para asociar formato.
fn strongest_match_id(matches: &[DuplicateMatch]) -> Option<u64> {
    matches
        .iter()
        .max_by_key(|m| m.certainty)
        .map(|m| m.catalog_id)
}

/// Catálogo mínimo necesario para el análisis; expuesto para la vista.
pub fn catalog_entry(id: u64, title: &str, author: &str, path: Option<String>) -> CatalogItem {
    CatalogItem::new(id, title, author, path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::import_scan::scan_paths;
    use std::path::Path;

    /// Lote temporal único por llamada: cargo test ejecuta los tests en
    /// paralelo dentro del mismo proceso, así que cada uno necesita su
    /// propio directorio para no pisarse.
    fn scan_temp_lote() -> (PathBuf, ScanOutcome) {
        use std::sync::atomic::{AtomicU32, Ordering};
        static SECUENCIA: AtomicU32 = AtomicU32::new(0);
        let numero = SECUENCIA.fetch_add(1, Ordering::Relaxed);
        let base = std::env::temp_dir().join(format!("mycalibro_wizard_{}_{}", std::process::id(), numero));
        std::fs::create_dir_all(&base).unwrap();
        std::fs::write(base.join("1984.epub"), b"x").unwrap();
        std::fs::write(base.join("nuevo.epub"), b"y").unwrap();
        let catalog = vec![CatalogItem::new(1, "1984", "George Orwell", Some(format!("{}", base.join("1984.epub").display())))];
        let outcome = scan_paths(&[base.clone()], &catalog);
        (base, outcome)
    }

    #[test]
    fn el_flujo_completo_prefilla_importacion_y_exige_revisar_duplicados() {
        let mut wizard = ImportWizard::new();
        assert_eq!(wizard.phase, Phase::Idle);
        let (base, outcome) = scan_temp_lote();
        wizard.start_scan(vec![base.clone()]);
        assert_eq!(wizard.phase, Phase::Scanning);
        wizard.record_progress(5, 2);
        assert_eq!(wizard.progress, ScanProgress { files_seen: 5, candidates_found: 2 });
        wizard.finish_scan(outcome);
        assert_eq!(wizard.phase, Phase::Reviewing);

        let summary = wizard.summary();
        assert_eq!(summary.candidates, 2);
        // «nuevo.epub» sin coincidencias: prefill Import. «1984.epub»: sin decidir.
        assert_eq!(summary.to_import, 1);
        assert_eq!(summary.undecided, 1);
        assert_eq!(summary.with_warnings, 1);
        assert!(wizard.build_plan().is_err(), "duplicado sin revisar bloquea el plan");
        std::fs::remove_dir_all(&base).unwrap();
    }

    #[test]
    fn decidir_el_duplicado_desbloquea_el_plan_y_respeta_omision() {
        let mut wizard = ImportWizard::new();
        let (base, outcome) = scan_temp_lote();
        wizard.finish_scan(outcome);
        let duplicado = wizard
            .undecided_duplicates()
            .iter()
            .map(|index| {
                let review = &wizard.outcome.as_ref().unwrap().reviews[*index];
                review.candidate.path.file_name().unwrap().to_string_lossy().to_string()
            })
            .find(|name| name == "1984.epub")
            .expect("debe existir el duplicado 1984.epub");
        let index = wizard
            .outcome
            .as_ref()
            .unwrap()
            .reviews
            .iter()
            .position(|r| r.candidate.path.file_name().unwrap().to_string_lossy() == duplicado)
            .unwrap();
        assert!(wizard.set_decision(index, Decision::Skip));
        let plan = wizard.build_plan().expect("con duplicados decididos el plan es válido");
        assert_eq!(plan.to_import, 1);
        assert_eq!(plan.to_skip, 1);
        assert!(plan.entries.iter().all(|entry| entry.decision == Decision::Import));
        std::fs::remove_dir_all(&base).unwrap();
    }

    #[test]
    fn asociar_formato_apunta_a_la_ficha_coincidente() {
        let mut wizard = ImportWizard::new();
        let (base, outcome) = scan_temp_lote();
        wizard.finish_scan(outcome);
        wizard.apply_suggestions();
        let index = wizard
            .outcome
            .as_ref()
            .unwrap()
            .reviews
            .iter()
            .position(|r| r.candidate.path.file_name().unwrap().to_string_lossy() == "1984.epub")
            .unwrap();
        // Sugerencia para duplicado de ruta exacta… la ruta del lote coincide
        // con la registrada en el catálogo, así que debe sugerir omitir;
        // probamos además la decisión explícita de asociar formato.
        assert_eq!(wizard.summary().undecided, 0, "apply_suggestions resuelve todo");
        assert!(wizard.set_decision(index, Decision::AssociateFormat));
        let plan = wizard.build_plan().unwrap();
        let entry = plan
            .entries
            .iter()
            .find(|entry| entry.decision == Decision::AssociateFormat)
            .expect("debe estar la entrada asociada");
        assert_eq!(entry.associate_with, Some(1));
        std::fs::remove_dir_all(&base).unwrap();
    }

    #[test]
    fn plan_vacio_o_sin_escaneo_devuelve_error_comprensible() {
        let mut wizard = ImportWizard::new();
        assert_eq!(
            wizard.build_plan().unwrap_err(),
            "Primero selecciona archivos o carpetas para importar"
        );
        let (base, outcome) = scan_temp_lote();
        wizard.finish_scan(outcome);
        // Todo omitido → sin trabajo.
        wizard.apply_suggestions();
        for index in 0..wizard.summary().candidates {
            wizard.set_decision(index, Decision::Skip);
        }
        assert_eq!(
            wizard.build_plan().unwrap_err(),
            "No hay ningún archivo pendiente de importar o asociar"
        );
        std::fs::remove_dir_all(&base).unwrap();
    }

    #[test]
    fn decision_fuera_de_rango_o_en_fase_incorrecta_se_rechaza() {
        let mut wizard = ImportWizard::new();
        assert!(!wizard.set_decision(0, Decision::Import), "sin escaneo no se decide");
        let (base, outcome) = scan_temp_lote();
        wizard.finish_scan(outcome);
        assert!(!wizard.set_decision(999, Decision::Import), "índice fuera de rango");
        assert!(wizard.set_decision(0, Decision::Import));
        std::fs::remove_dir_all(&base).unwrap();
    }

    #[test]
    fn start_scan_reinicia_estado_anterior() {
        let mut wizard = ImportWizard::new();
        let (base, outcome) = scan_temp_lote();
        wizard.finish_scan(outcome);
        assert_eq!(wizard.phase, Phase::Reviewing);
        wizard.start_scan(vec![Path::new("C:\\otra").to_path_buf()]);
        assert_eq!(wizard.phase, Phase::Scanning);
        assert!(wizard.outcome.is_none());
        assert!(wizard.decisions.is_empty());
        assert_eq!(wizard.progress, ScanProgress::default());
        std::fs::remove_dir_all(&base).unwrap();
    }

    #[test]
    fn certezas_y_coincidencias_se_explican_para_la_interfaz() {
        // Utilidad de contrato: la vista debe poder mostrar kind+certainty.
        let matches = vec![DuplicateMatch {
            catalog_id: 7,
            title: "Título".to_owned(),
            author: "Autor".to_owned(),
            path: None,
            kind: MatchKind::TitleAuthorSimilar,
            certainty: Certainty::Low,
        }];
        assert_eq!(strongest_match_id(&matches), Some(7));
    }
}
