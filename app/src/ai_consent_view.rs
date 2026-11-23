//! Vista egui del consentimiento IA por libro (PROD-006).
//!
//! Diálogo de autorización y fila de estado/revocación para la ficha del
//! libro. Toda la lógica vive en [`crate::ai_consent`]; esta vista solo
//! muestra el aviso exacto, concede, deniega y revoca.
//!
//! Reglas de producto reflejadas aquí:
//! - La autorización se pide cuando el usuario pide un análisis, no por el
//!   mero hecho de estar el libro en la biblioteca.
//! - El aviso muestra la cadena completa de proveedores (principal y
//!   respaldo) antes de conceder; el conjunto concedido es exactamente el
//!   que se muestra.
//! - Denegar no simula análisis: la app verá `ConsentError::NoConsent` de
//!   [`ConsentManager::can_send_text`] y comunicará que no hay análisis de
//!   contenido sin autorización.
//!
//! Contrato con la aplicación (integración en main.rs): la app posee el
//! [`ConsentManager`] (persistido con su capa de almacenamiento) y llama a
//! `request_consent` cuando el usuario solicita un análisis sin autorización
//! vigente; cada fotograma llama a `show(ctx, manager)` y consulta
//! `pending_id()` para saber si el análisis debe esperar.

// El diálogo se activa cuando el usuario pide un análisis; el motor de
// análisis por libro está pendiente de integración (issue #1).
#![allow(dead_code)]

use eframe::egui;

use crate::ai_consent::{consent_notice, ConsentManager, ConsentStatus, Provider};

/// Cadena de proveedores de la aplicación: Gemini principal, Groq de
/// respaldo (igual orden que la clasificación existente). Debe pasarse tal
/// cual a grant para que el conjunto autorizado sea el mostrado.
pub const PROVIDER_CHAIN: &[Provider] = &[Provider::Gemini, Provider::Groq];

/// Límite propuesto de caracteres por envío (minimización, PROD-006).
pub const DEFAULT_FRAGMENT_CHARS: usize = 6000;

/// Vista del diálogo de consentimiento.
pub struct AiConsentView {
    pending: Option<PendingConsent>,
    mensaje: Option<String>,
}

#[derive(Clone, Debug)]
struct PendingConsent {
    book_id: u64,
    book_title: String,
    fragment_chars: usize,
}

impl AiConsentView {
    pub fn new() -> Self {
        Self {
            pending: None,
            mensaje: None,
        }
    }

    /// Pide autorización para analizar el contenido del libro. Solo debe
    /// llamarse cuando el usuario ha solicitado un análisis.
    pub fn request_consent(&mut self, book_id: u64, book_title: String) {
        if self.pending.is_none() {
            self.pending = Some(PendingConsent {
                book_id,
                book_title,
                fragment_chars: DEFAULT_FRAGMENT_CHARS,
            });
        }
    }

    /// Libro esperando decisión, si lo hay (el análisis queda a la espera).
    pub fn pending_id(&self) -> Option<u64> {
        self.pending.as_ref().map(|p| p.book_id)
    }

    /// Concede la autorización pendiente con la cadena mostrada.
    pub fn authorize_pending(&mut self, manager: &mut ConsentManager) -> bool {
        let Some(pending) = self.pending.take() else {
            return false;
        };
        manager.grant_text_consent(pending.book_id, PROVIDER_CHAIN, pending.fragment_chars);
        self.mensaje = Some(format!(
            "Autorizado para «{}». Puedes revocarlo desde su ficha.",
            pending.book_title
        ));
        true
    }

    /// Deniega la autorización pendiente. No se concede nada y el análisis
    /// de contenido no podrá ejecutarse para este libro sin consentimiento.
    pub fn deny_pending(&mut self) -> bool {
        let Some(pending) = self.pending.take() else {
            return false;
        };
        self.mensaje = Some(format!(
            "Sin autorización, «{}» no enviará contenido a ningún servicio: \
             el análisis basado en texto no está disponible.",
            pending.book_title
        ));
        true
    }

    /// Mensaje informativo para la interfaz (resultado de la decisión).
    pub fn take_message(&mut self) -> Option<String> {
        self.mensaje.take()
    }

    /// Dibuja el diálogo si hay una petición pendiente.
    pub fn show(&mut self, ctx: &egui::Context, manager: &mut ConsentManager) {
        let Some(pending) = self.pending.as_ref() else {
            return;
        };
        let (book_id, titulo, fragment_chars) = (
            pending.book_id,
            pending.book_title.clone(),
            pending.fragment_chars,
        );
        let ya_autorizado = matches!(manager.status(book_id), ConsentStatus::Authorized(_));
        let mut decision: Option<bool> = None;
        egui::Window::new("Enviar contenido a IA")
            .collapsible(false)
            .resizable(false)
            .show(ctx, |ui| {
                ui.label(format!("Libro: «{titulo}»"));
                ui.add_space(6.0);
                ui.label(consent_notice(PROVIDER_CHAIN, fragment_chars));
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    if ui.button("Autorizar análisis").clicked() {
                        decision = Some(true);
                    }
                    if ui.button("No autorizar").clicked() {
                        decision = Some(false);
                    }
                });
                if ya_autorizado {
                    ui.weak("Este libro ya tiene autorización vigente; concede solo si quieres actualizarla.");
                }
            });
        match decision {
            Some(true) => {
                self.authorize_pending(manager);
            }
            Some(false) => {
                self.deny_pending();
            }
            None => {}
        }
    }
}

impl Default for AiConsentView {
    fn default() -> Self {
        Self::new()
    }
}

/// Fila de estado y revocación para la ficha del libro (integración de la
/// vista de detalle): muestra qué está autorizado y permite revocar.
/// Devuelve true si el usuario acaba de revocar.
pub fn status_row(ui: &mut egui::Ui, manager: &mut ConsentManager, book_id: u64) -> bool {
    let mut revocado = false;
    match manager.status(book_id) {
        ConsentStatus::MetadataOnly => {
            ui.weak("IA: solo clasificación por metadatos; sin envío de contenido.");
        }
        ConsentStatus::Authorized(consent) => {
            let proveedores: Vec<String> = consent
                .providers
                .iter()
                .map(|p| p.display_name().to_owned())
                .collect();
            ui.label(format!(
                "IA: contenido autorizado para {} ({} caracteres por envío).",
                proveedores.join(" y "),
                consent.max_fragment_chars
            ));
            if ui.small_button("Revocar autorización").clicked() {
                manager.revoke(book_id);
                revocado = true;
            }
        }
    }
    revocado
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai_consent::{ConsentError, TextRequest, CONSENT_SCOPE_VERSION};

    #[test]
    fn pedir_autorizar_y_conceder_registra_la_cadena_mostrada() {
        let mut view = AiConsentView::new();
        let mut manager = ConsentManager::new();
        assert!(view.pending_id().is_none());
        view.request_consent(11, "Moby Dick".to_owned());
        assert_eq!(view.pending_id(), Some(11));
        assert!(view.authorize_pending(&mut manager));
        assert!(view.pending_id().is_none(), "decisión tomada");
        // Ambos proveedores de la cadena mostrada quedan autorizados.
        for proveedor in PROVIDER_CHAIN {
            assert_eq!(
                manager.can_send_text(
                    11,
                    TextRequest {
                        provider: *proveedor,
                        chars_to_send: DEFAULT_FRAGMENT_CHARS,
                        scope_version: CONSENT_SCOPE_VERSION,
                    }
                ),
                Ok(())
            );
        }
        // Y un tercero no autorizado (si existiera) no lo estaría: la cadena
        // exacta se registra, no un «cualquiera».
        assert!(view.take_message().is_some());
    }

    #[test]
    fn denegar_no_concede_nada_y_explica_la_consecuencia() {
        let mut view = AiConsentView::new();
        let mut manager = ConsentManager::new();
        view.request_consent(12, "Ulises".to_owned());
        assert!(view.deny_pending());
        assert_eq!(view.pending_id(), None);
        let request = TextRequest {
            provider: Provider::Gemini,
            chars_to_send: 1000,
            scope_version: CONSENT_SCOPE_VERSION,
        };
        assert_eq!(
            manager.can_send_text(12, request),
            Err(ConsentError::NoConsent),
            "denegar no concede"
        );
        let mensaje = view.take_message().expect("mensaje para el usuario");
        assert!(mensaje.contains("no enviará contenido"));
    }

    #[test]
    fn solo_hay_una_peticion_pendiente_a_la_vez() {
        let mut view = AiConsentView::new();
        view.request_consent(1, "A".to_owned());
        view.request_consent(2, "B".to_owned());
        assert_eq!(
            view.pending_id(),
            Some(1),
            "la primera pendiente no se pisa"
        );
        let mut manager = ConsentManager::new();
        assert!(view.authorize_pending(&mut manager));
        assert_eq!(manager.status(2), ConsentStatus::MetadataOnly);
    }

    #[test]
    fn la_fila_de_estado_revoca_y_avisa_sin_autorizacion() {
        let mut manager = ConsentManager::new();
        manager.grant_text_consent(20, PROVIDER_CHAIN, DEFAULT_FRAGMENT_CHARS);
        // Con autorización, la fila no revoca sin clic: aquí se comprueba el
        // efecto del revoke directo que la fila dispara al pulsar el botón.
        assert!(
            manager.can_send_text(
                20,
                TextRequest {
                    provider: Provider::Gemini,
                    chars_to_send: 1000,
                    scope_version: CONSENT_SCOPE_VERSION,
                }
            ) == Ok(())
        );
        manager.revoke(20);
        assert_eq!(manager.status(20), ConsentStatus::MetadataOnly);
    }
}
