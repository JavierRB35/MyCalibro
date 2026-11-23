//! Contratos de proveedores IA. Las pruebas de conexión no envían datos de libros.
use reqwest::blocking::{Client, Response};
use serde_json::{json, Value};
use std::time::{Duration, Instant};

pub const GEMINI_MODEL: &str = "gemini-3.5-flash-lite";
pub const GROQ_MODEL: &str = "openai/gpt-oss-20b";

pub fn gemini_endpoint() -> String {
    format!(
        "https://generativelanguage.googleapis.com/v1beta/models/{GEMINI_MODEL}:generateContent"
    )
}

pub fn groq_analysis_format() -> Value {
    json!({
        "type": "json_schema",
        "json_schema": {
            "name": "mycalibro_analysis",
            "strict": true,
            "schema": {
                "type": "object",
                "properties": {
                    "answer": {"type": "string"},
                    "supported": {"type": "boolean"},
                    "citations": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "properties": {
                                "fragment_id": {"type": "string"},
                                "quote": {"type": "string"}
                            },
                            "required": ["fragment_id", "quote"],
                            "additionalProperties": false
                        }
                    }
                },
                "required": ["answer", "supported", "citations"],
                "additionalProperties": false
            }
        }
    })
}

pub fn groq_classification_format() -> Value {
    json!({
        "type": "json_schema",
        "json_schema": {
            "name": "mycalibro_classification",
            "strict": true,
            "schema": {
                "type": "object",
                "properties": {
                    "topic": {"type": "string"},
                    "tags": {"type": "array", "items": {"type": "string"}}
                },
                "required": ["topic", "tags"],
                "additionalProperties": false
            }
        }
    })
}

/// Error sin exponer credenciales ni la respuesta completa del servidor.
pub fn read_json_response(
    response: Response,
    provider: &str,
    model: &str,
) -> Result<Value, String> {
    let code = response.status().as_u16();
    if !response.status().is_success() {
        let reason = match code {
            400 => "Petición rechazada; revise el formato o las opciones del modelo",
            401 => "Clave API no autorizada",
            403 => "El proyecto no tiene permiso para utilizar este modelo",
            404 => "Modelo no disponible para este proyecto o endpoint incorrecto",
            429 => "Cuota o límite de peticiones alcanzado",
            500..=599 => "Servicio temporalmente no disponible",
            _ => "Solicitud rechazada por el proveedor",
        };
        return Err(format!("{provider} ({model}): HTTP {code}. {reason}"));
    }
    response
        .json::<Value>()
        .map_err(|_| format!("{provider} ({model}): respuesta HTTP sin JSON válido"))
}

/// Comprobación independiente y no destructiva con una pregunta sin contenido privado.
pub fn test_connection(provider: &str, api_key: &str) -> Result<u128, String> {
    if api_key.trim().is_empty() {
        return Err(format!("No hay una clave de {provider} guardada"));
    }
    let client = Client::builder()
        .timeout(Duration::from_secs(30))
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|e| e.to_string())?;
    let start = Instant::now();
    match provider {
        "Gemini" => {
            let response = client
                .post(gemini_endpoint())
                .header("x-goog-api-key", api_key)
                .json(&json!({
                    "contents": [{"parts": [{"text": "Responde OK, sin explicación."}]}],
                    "generationConfig": {"maxOutputTokens": 100}
                }))
                .send()
                .map_err(|e| format!("Gemini: conexión fallida: {e}"))?;
            let body = read_json_response(response, provider, GEMINI_MODEL)?;
            let output = body["candidates"][0]["content"]["parts"]
                .as_array()
                .map(|parts| {
                    parts
                        .iter()
                        .filter_map(|p| p["text"].as_str())
                        .collect::<String>()
                })
                .unwrap_or_default();
            if output.trim().is_empty() {
                return Err("Gemini respondió sin texto verificable".into());
            }
        }
        "Groq" => {
            let response = client
                .post("https://api.groq.com/openai/v1/chat/completions")
                .bearer_auth(api_key)
                .json(&json!({
                    "model": GROQ_MODEL,
                    "messages": [{"role": "user", "content": "Responde OK, sin explicación."}],
                    "reasoning_effort": "low",
                    "max_completion_tokens": 256
                }))
                .send()
                .map_err(|e| format!("Groq: conexión fallida: {e}"))?;
            let body = read_json_response(response, provider, GROQ_MODEL)?;
            if body["choices"][0]["message"]["content"]
                .as_str()
                .is_none_or(|content| content.trim().is_empty())
            {
                return Err("Groq respondió sin texto verificable".into());
            }
        }
        _ => return Err("Proveedor de IA desconocido".into()),
    }
    Ok(start.elapsed().as_millis())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn groq_strict_schema_matches_analysis_parser_contract() {
        let value = groq_analysis_format();
        assert_eq!(value["type"], "json_schema");
        assert_eq!(value["json_schema"]["strict"], true);
        let schema = &value["json_schema"]["schema"];
        assert_eq!(schema["additionalProperties"], false);
        assert_eq!(
            schema["required"],
            json!(["answer", "supported", "citations"])
        );
        assert_eq!(
            schema["properties"]["citations"]["items"]["required"],
            json!(["fragment_id", "quote"])
        );
        assert_eq!(
            schema["properties"]["citations"]["items"]["additionalProperties"],
            false
        );
    }

    #[test]
    fn classification_schema_forbids_unexpected_fields() {
        let value = groq_classification_format();
        let schema = &value["json_schema"]["schema"];
        assert_eq!(schema["additionalProperties"], false);
        assert_eq!(schema["required"], json!(["topic", "tags"]));
    }

    #[test]
    fn missing_keys_never_issue_network_requests() {
        assert!(test_connection("Gemini", "")
            .unwrap_err()
            .contains("guardada"));
        assert!(test_connection("Groq", " ")
            .unwrap_err()
            .contains("guardada"));
        assert!(gemini_endpoint().contains(GEMINI_MODEL));
        assert!(!gemini_endpoint().contains("2.5-flash-lite"));
    }
}
