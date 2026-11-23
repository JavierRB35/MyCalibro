//! Consentimiento por libro para el envío de contenido a IA externa
//! (PROD-005 y PROD-006).
//!
//! Parte pura, sin egui: registra qué se autorizó para cada libro (proveedor
//! visible incluido el de respaldo, categoría de datos y finalidad), exige
//! consentimiento nuevo cuando cambia el alcance y bloquea cualquier envío
//! de texto sin autorización vigente. La clasificación por metadatos que ya
//! existe en la aplicación es distinta y no requiere este consentimiento.
//!
//! Contrato de integración: la vista muestra [`consent_notice`] antes de
//! pedir autorización, llama a [`ConsentManager::grant_text_consent`] y
//! consulta [`ConsentManager::can_send_text`] antes de cada envío. La
//! persistencia corre a cargo de la capa existente mediante serde; el mapa
//! usa el identificador estable del libro y no contiene secretos.

// La API de envío (can_send_text, TextRequest, ConsentError, AnalysisSource)
// queda a la espera del motor de análisis por libro; el estado, el aviso y
// la fila de la ficha ya están cableados. Integración trazada en el issue
// de coordinación #1.
#![allow(dead_code)]

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// Versión del alcance autorizado. Si la aplicación cambia qué categorías de
/// datos envía o qué proveedores usan los análisis, debe aumentar esta
/// versión para forzar un consentimiento nuevo (PROD-006, información
/// transparente).
pub const CONSENT_SCOPE_VERSION: u32 = 1;

/// Proveedores de IA visibles para el usuario.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Provider {
    /// Principal para clasificación y análisis.
    Gemini,
    /// Respaldo si el principal no está disponible o falla.
    Groq,
}

impl Provider {
    pub fn display_name(self) -> &'static str {
        match self {
            Provider::Gemini => "Google Gemini",
            Provider::Groq => "Groq",
        }
    }
}

/// Lo que el usuario autorizó para un libro concreto.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Consent {
    /// Conjunto EXACTO de proveedores que el aviso mostró al autorizar
    /// (principal y respaldo). Un envío a un destinatario fuera de este
    /// conjunto se rechaza (hallazgo del AGENTE A sobre af2446d).
    pub providers: BTreeSet<Provider>,
    /// Versión del alcance vigente cuando se concedió.
    pub scope_version: u32,
    /// Máximo de caracteres de texto por envío autorizado (minimización).
    pub max_fragment_chars: usize,
}

/// Solicitud de envío de texto evaluada contra el consentimiento vigente.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TextRequest {
    pub provider: Provider,
    pub chars_to_send: usize,
    /// Versión de alcance que la aplicación aplicaría a este envío.
    pub scope_version: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConsentError {
    /// Sin autorización vigente: la interfaz debe pedirla antes de enviar.
    NoConsent,
    /// Autorizado, pero con un alcance distinto al actual: hace falta
    /// consentimiento nuevo.
    ScopeChanged,
    /// Autorizado, pero la petición excede el tamaño autorizado: reducir
    /// fragmentos o pedir consentimiento nuevo.
    FragmentTooLarge,
    /// El destinatario concreto del envío no estaba entre los proveedores
    /// mostrados al autorizar: prohibido (PROD-006, destinatarios
    /// transparentes).
    ProviderNotAuthorized,
}

/// Estado de autorización de un libro, para mostrarlo en la ficha.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ConsentStatus {
    /// Solo la clasificación por metadatos existente; sin envío de contenido.
    MetadataOnly,
    /// Contenido autorizado bajo estas condiciones.
    Authorized(Consent),
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConsentManager {
    #[serde(default)]
    consents: BTreeMap<u64, Consent>,
}

impl ConsentManager {
    pub fn new() -> Self {
        Self::default()
    }

    /// Estado de autorización del libro (para la ficha y para decidir si la
    /// interfaz debe pedir permiso).
    pub fn status(&self, book_id: u64) -> ConsentStatus {
        self.consents
            .get(&book_id)
            .map(|c| ConsentStatus::Authorized(c.clone()))
            .unwrap_or(ConsentStatus::MetadataOnly)
    }

    /// Registra la autorización de texto para el libro. La vista debe haber
    /// mostrado [`consent_notice`] con EXACTAMENTE la misma cadena de
    /// proveedores que se registra aquí: el conjunto queda persistido y solo
    /// esos destinatarios podrán recibir contenido.
    pub fn grant_text_consent(
        &mut self,
        book_id: u64,
        providers: &[Provider],
        max_fragment_chars: usize,
    ) {
        self.consents.insert(
            book_id,
            Consent {
                providers: providers.iter().copied().collect(),
                scope_version: CONSENT_SCOPE_VERSION,
                max_fragment_chars,
            },
        );
    }

    /// Revoca la autorización del libro. A partir de aquí ningún envío puede
    /// ampararse en el consentimiento anterior (PROD-006).
    pub fn revoke(&mut self, book_id: u64) {
        self.consents.remove(&book_id);
    }

    /// Alias semántico para el borrado del libro: nada que conservar.
    pub fn forget(&mut self, book_id: u64) {
        self.revoke(book_id);
    }

    /// Evalúa si un envío de texto puede proceder bajo el consentimiento
    /// vigente. Debe consultarse antes de CADA envío, incluido el fallback
    /// al proveedor de respaldo (la revocación debe valer para el siguiente
    /// envío, no solo al abrir el panel). La clasificación por metadatos
    /// (título, autor, descripción, etiquetas) no pasa por aquí.
    pub fn can_send_text(&self, book_id: u64, request: TextRequest) -> Result<(), ConsentError> {
        let Some(consent) = self.consents.get(&book_id) else {
            return Err(ConsentError::NoConsent);
        };
        if consent.scope_version != request.scope_version {
            return Err(ConsentError::ScopeChanged);
        }
        if !consent.providers.contains(&request.provider) {
            return Err(ConsentError::ProviderNotAuthorized);
        }
        if request.chars_to_send > consent.max_fragment_chars {
            return Err(ConsentError::FragmentTooLarge);
        }
        Ok(())
    }

    /// Libros con autorización vigente (para «revisar y revocar» en bloque).
    pub fn authorized_book_ids(&self) -> Vec<u64> {
        self.consents.keys().copied().collect()
    }
}

/// Aviso exacto que la interfaz debe mostrar antes de pedir autorización
/// (PROD-006): proveedor principal y de respaldo, categoría de datos,
/// finalidad y minimización. `fragment_chars` es el límite propuesto.
pub fn consent_notice(chain: &[Provider], fragment_chars: usize) -> String {
    let proveedores = match chain {
        [] => "un servicio de IA externo".to_owned(),
        [unico] => unico.display_name().to_owned(),
        [principal, resto @ ..] => {
            let lista: Vec<String> = resto.iter().map(|p| p.display_name().to_owned()).collect();
            format!(
                "{} (respaldo: {})",
                principal.display_name(),
                lista.join(", ")
            )
        }
    };
    format!(
        "Para analizar «contenido del libro» se enviarán fragmentos de su texto \
         (máximo {fragment_chars} caracteres por consulta) a {proveedores}, con la \
         finalidad de generar el resumen, análisis o respuesta que solicites. \
         No se envía el libro completo. La clasificación por título, autor, \
         descripción y etiquetas ya existente no envía contenido. Puedes revocar \
         esta autorización en cualquier momento desde la ficha del libro; la \
         revocación no retira los datos ya tratados por el proveedor."
    )
}

/// Etiqueta honesta para presentar un resultado de IA según su fuente
/// (PROD-006: sin simular análisis de texto sin permiso).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AnalysisSource {
    /// Respuesta basada solo en metadatos (sin contenido del libro).
    MetadataOnly,
    /// Respuesta basada en fragmentos autorizados del contenido.
    TextFragments,
}

impl AnalysisSource {
    pub fn label(self) -> &'static str {
        match self {
            AnalysisSource::MetadataOnly => "Basado solo en metadatos (no se envió contenido)",
            AnalysisSource::TextFragments => "Basado en fragmentos del contenido autorizados",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    fn conjunto(proveedores: &[Provider]) -> BTreeSet<Provider> {
        proveedores.iter().copied().collect()
    }

    #[test]
    fn sin_consentimiento_se_bloquea_y_con_el_se_permite() {
        let mut manager = ConsentManager::new();
        let request = TextRequest {
            provider: Provider::Gemini,
            chars_to_send: 4000,
            scope_version: CONSENT_SCOPE_VERSION,
        };
        assert_eq!(
            manager.can_send_text(1, request),
            Err(ConsentError::NoConsent)
        );
        assert_eq!(manager.status(1), ConsentStatus::MetadataOnly);

        manager.grant_text_consent(1, &[Provider::Gemini, Provider::Groq], 8000);
        assert_eq!(manager.can_send_text(1, request), Ok(()));
        // El respaldo está autorizado porque el aviso mostró la cadena entera.
        assert_eq!(
            manager.can_send_text(
                1,
                TextRequest {
                    provider: Provider::Groq,
                    ..request
                }
            ),
            Ok(())
        );
        let mut esperado = BTreeSet::new();
        esperado.insert(Provider::Gemini);
        esperado.insert(Provider::Groq);
        assert_eq!(
            manager.status(1),
            ConsentStatus::Authorized(Consent {
                providers: esperado,
                scope_version: CONSENT_SCOPE_VERSION,
                max_fragment_chars: 8000,
            })
        );
    }

    #[test]
    fn destinatario_fuera_del_conjunto_autorizado_se_bloquea() {
        // Hallazgo del AGENTE A sobre af2446d: autorizar viendo solo Groq y
        // enviar después a Gemini no puede colar.
        let mut manager = ConsentManager::new();
        manager.grant_text_consent(5, &[Provider::Groq], 6000);
        assert_eq!(
            manager.can_send_text(
                5,
                TextRequest {
                    provider: Provider::Groq,
                    chars_to_send: 4000,
                    scope_version: CONSENT_SCOPE_VERSION,
                }
            ),
            Ok(())
        );
        assert_eq!(
            manager.can_send_text(
                5,
                TextRequest {
                    provider: Provider::Gemini,
                    chars_to_send: 4000,
                    scope_version: CONSENT_SCOPE_VERSION,
                }
            ),
            Err(ConsentError::ProviderNotAuthorized)
        );
        // Y a la inversa: autorizando solo Gemini, Groq queda bloqueado.
        let mut solo_gemini = ConsentManager::new();
        solo_gemini.grant_text_consent(6, &[Provider::Gemini], 6000);
        assert_eq!(
            solo_gemini.can_send_text(
                6,
                TextRequest {
                    provider: Provider::Groq,
                    chars_to_send: 1000,
                    scope_version: CONSENT_SCOPE_VERSION,
                }
            ),
            Err(ConsentError::ProviderNotAuthorized)
        );
    }

    #[test]
    fn revocar_bloquea_los_envios_posteriores() {
        let mut manager = ConsentManager::new();
        manager.grant_text_consent(2, &[Provider::Groq], 5000);
        let request = TextRequest {
            provider: Provider::Groq,
            chars_to_send: 4000,
            scope_version: CONSENT_SCOPE_VERSION,
        };
        assert_eq!(manager.can_send_text(2, request), Ok(()));
        manager.revoke(2);
        assert_eq!(
            manager.can_send_text(2, request),
            Err(ConsentError::NoConsent)
        );
        assert_eq!(manager.status(2), ConsentStatus::MetadataOnly);
        // Revocar de nuevo no es error.
        manager.revoke(2);
    }

    #[test]
    fn cambio_de_alcance_exige_consentimiento_nuevo() {
        let mut manager = ConsentManager::new();
        manager.grant_text_consent(3, &[Provider::Gemini], 8000);
        let version_nueva = CONSENT_SCOPE_VERSION + 1;
        let request = TextRequest {
            provider: Provider::Gemini,
            chars_to_send: 4000,
            scope_version: version_nueva,
        };
        assert_eq!(
            manager.can_send_text(3, request),
            Err(ConsentError::ScopeChanged)
        );
        // Con la versión autorizada sí se evalúa el tamaño: excederlo rechaza.
        assert_eq!(
            manager.can_send_text(
                3,
                TextRequest {
                    chars_to_send: 9000,
                    scope_version: CONSENT_SCOPE_VERSION,
                    ..request
                }
            ),
            Err(ConsentError::FragmentTooLarge)
        );
    }

    #[test]
    fn borrado_del_libro_olvida_la_autorizacion() {
        let mut manager = ConsentManager::new();
        manager.grant_text_consent(4, &[Provider::Gemini], 6000);
        manager.forget(4);
        assert_eq!(manager.status(4), ConsentStatus::MetadataOnly);
        assert!(manager.authorized_book_ids().is_empty());
    }

    #[test]
    fn el_aviso_muestra_proveedores_cadena_datos_finalidad_y_revocacion() {
        let aviso = consent_notice(&[Provider::Gemini, Provider::Groq], 6000);
        assert!(aviso.contains("Google Gemini"));
        assert!(aviso.contains("respaldo: Groq"));
        assert!(aviso.contains("fragmentos"));
        assert!(aviso.contains("No se envía el libro completo"));
        assert!(aviso.contains("revocar"));
        // Sin cadena no se inventa proveedor.
        let vacio = consent_notice(&[], 1000);
        assert!(vacio.contains("un servicio de IA externo"));
    }

    #[test]
    fn las_etiquetas_de_fuente_distinguen_metadatos_de_contenido() {
        assert!(AnalysisSource::MetadataOnly
            .label()
            .contains("no se envió contenido"));
        assert!(AnalysisSource::TextFragments.label().contains("fragmentos"));
    }

    #[test]
    fn el_estado_persiste_por_serde_sin_secretos() {
        let mut manager = ConsentManager::new();
        manager.grant_text_consent(9, &[Provider::Groq], 7000);
        let json = serde_json::to_string(&manager).unwrap();
        assert!(
            !json.to_lowercase().contains("key"),
            "no hay claves en el estado: {json}"
        );
        let recuperado: ConsentManager = serde_json::from_str(&json).unwrap();
        assert_eq!(
            recuperado.status(9),
            manager.status(9),
            "autorización sobrevive al ciclo de persistencia"
        );
        // La cadena exacta persistida conserva ambos destinatarios.
        let mut ambos = ConsentManager::new();
        ambos.grant_text_consent(10, &[Provider::Gemini, Provider::Groq], 7000);
        let json_ambos = serde_json::to_string(&ambos).unwrap();
        let vuelta: ConsentManager = serde_json::from_str(&json_ambos).unwrap();
        assert_eq!(
            vuelta.can_send_text(
                10,
                TextRequest {
                    provider: Provider::Groq,
                    chars_to_send: 1000,
                    scope_version: CONSENT_SCOPE_VERSION,
                }
            ),
            Ok(())
        );
        assert_eq!(
            vuelta.can_send_text(
                10,
                TextRequest {
                    provider: Provider::Gemini,
                    chars_to_send: 1000,
                    scope_version: CONSENT_SCOPE_VERSION,
                }
            ),
            Ok(())
        );
    }
}
