//! Local extraction and bounded, traceable excerpts. No network access.
use serde::Serialize;
use std::{
    io::Read,
    path::Path,
    process::{Command, Stdio},
    time::{Duration, Instant},
};

#[derive(Clone, Debug, Serialize)]
pub struct Fragment {
    pub id: String,
    pub location: String,
    pub text: String,
}

const MAX_LOCAL_BYTES: u64 = 16 * 1024 * 1024;
pub const MAX_SEND_CHARS: usize = 6000;

fn read_text(path: &Path) -> Result<String, String> {
    let mut bytes = Vec::new();
    std::fs::File::open(path)
        .map_err(|e| e.to_string())?
        .take(MAX_LOCAL_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() as u64 > MAX_LOCAL_BYTES {
        return Err(
            "El texto supera el límite local de análisis (16 MiB); no se ha enviado nada".into(),
        );
    }
    String::from_utf8(bytes)
        .map(|s| s.trim_start_matches('\u{feff}').replace("\r\n", "\n"))
        .map_err(|_| "El texto no está codificado en UTF-8; conviértalo antes de analizarlo".into())
}

fn converted_text(source: &Path, converter: &Path) -> Result<String, String> {
    let temporary = tempfile::tempdir().map_err(|e| e.to_string())?;
    let target = temporary.path().join("extracted.txt");
    let mut child = Command::new(converter)
        .arg(source)
        .arg(&target)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| format!("No se pudo iniciar la extracción local: {e}"))?;
    let start = Instant::now();
    loop {
        match child.try_wait().map_err(|e| e.to_string())? {
            Some(status) if status.success() => return read_text(&target),
            Some(_) => {
                return Err(
                    "Calibre no pudo extraer texto de este formato. No se ha enviado contenido"
                        .into(),
                )
            }
            None if start.elapsed() > Duration::from_secs(120) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(
                    "La extracción local excedió dos minutos; se ha detenido sin enviar contenido"
                        .into(),
                );
            }
            None => std::thread::sleep(Duration::from_millis(50)),
        }
    }
}

pub fn extract(path: &Path, converter: Option<&Path>) -> Result<Vec<Fragment>, String> {
    let extension = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    let paragraphs = if extension == "epub" {
        if std::fs::metadata(path).map_err(|e| e.to_string())?.len() > 64 * 1024 * 1024 {
            return Err("EPUB demasiado grande para análisis local; no se ha enviado nada".into());
        }
        {
            let mut archive =
                zip::ZipArchive::new(std::fs::File::open(path).map_err(|e| e.to_string())?)
                    .map_err(|e| e.to_string())?;
            let mut expanded = 0u64;
            for index in 0..archive.len() {
                let entry = archive.by_index(index).map_err(|e| e.to_string())?;
                expanded = expanded.saturating_add(entry.size());
                if entry.size() > MAX_LOCAL_BYTES || expanded > 128 * 1024 * 1024 {
                    return Err(
                        "El EPUB supera el límite de extracción local; no se ha enviado nada"
                            .into(),
                    );
                }
            }
        }
        let document = crate::reader_epub::EpubDocument::from_path(path)?;
        document
            .chapters
            .into_iter()
            .enumerate()
            .flat_map(|(chapter, c)| {
                c.paragraphs
                    .into_iter()
                    .enumerate()
                    .map(move |(paragraph, text)| {
                        (
                            format!("Capítulo {}, párrafo {}", chapter + 1, paragraph + 1),
                            text,
                        )
                    })
            })
            .collect::<Vec<_>>()
    } else {
        let text = if extension == "txt" {
            read_text(path)?
        } else {
            converted_text(path,converter.ok_or("Este formato requiere ebook-convert de Calibre para extraer texto; puede consultar solo metadatos")?)?
        };
        text.split("\n\n")
            .enumerate()
            .filter(|(_, p)| !p.trim().is_empty())
            .map(|(i, p)| {
                (
                    format!("Texto extraído, párrafo {}", i + 1),
                    p.trim().to_owned(),
                )
            })
            .collect()
    };
    let mut fragments = Vec::new();
    let mut local_chars = 0usize;
    for (location, text) in paragraphs {
        local_chars += text.chars().count();
        if local_chars > MAX_LOCAL_BYTES as usize {
            return Err(
                "El contenido extraído supera el límite local; no se ha enviado nada".into(),
            );
        }
        let chars: Vec<_> = text.chars().collect();
        for (part, chunk) in chars.chunks(1500).enumerate() {
            let text: String = chunk.iter().collect();
            if text.trim().is_empty() {
                continue;
            }
            fragments.push(Fragment {
                id: format!("F{}", fragments.len() + 1),
                location: format!("{location}, segmento {}", part + 1),
                text,
            });
        }
    }
    if fragments.is_empty() {
        return Err("No se encontró texto legible; no se ha enviado contenido".into());
    }
    Ok(fragments)
}

pub fn select(fragments: &[Fragment], question: &str) -> Result<Vec<Fragment>, String> {
    let total: usize = fragments.iter().map(|f| f.text.chars().count()).sum();
    // Always retain a local remainder; the API receives excerpts, never the whole text.
    let mut remaining = MAX_SEND_CHARS.min(total.saturating_sub(1));
    if remaining == 0 {
        return Err("No hay texto suficiente para seleccionar fragmentos".into());
    }
    let words: Vec<_> = question
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| w.chars().count() > 3)
        .map(str::to_lowercase)
        .collect();
    let mut ranked: Vec<_> = fragments
        .iter()
        .enumerate()
        .map(|(i, f)| {
            let text = f.text.to_lowercase();
            (
                i,
                words.iter().filter(|w| text.contains(w.as_str())).count(),
            )
        })
        .collect();
    ranked.sort_by_key(|(i, score)| (std::cmp::Reverse(*score), *i));
    let indices = if ranked.first().is_some_and(|(_, score)| *score > 0) {
        ranked
            .into_iter()
            .map(|(i, _)| i)
            .take(4)
            .collect::<Vec<_>>()
    } else {
        let n = fragments.len();
        vec![0, n / 3, 2 * n / 3, n - 1]
    };
    let mut selected = Vec::new();
    let mut seen = std::collections::BTreeSet::new();
    for index in indices {
        if remaining == 0 {
            break;
        }
        if !seen.insert(index) {
            continue;
        }
        let mut fragment = fragments[index].clone();
        fragment.text = fragment.text.chars().take(remaining.min(1500)).collect();
        remaining -= fragment.text.chars().count();
        selected.push(fragment);
    }
    Ok(selected)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unicode_excerpts_are_bounded_relevant_and_never_include_the_whole_book() {
        let fragments = (0..12)
            .map(|i| Fragment {
                id: format!("F{i}"),
                location: format!("Párrafo {i}"),
                text: if i == 7 {
                    "Astronomía lunar. ".repeat(100)
                } else {
                    "Texto común. ".repeat(200)
                },
            })
            .collect::<Vec<_>>();
        let selected = select(&fragments, "astronomía lunar").unwrap();
        assert_eq!(selected[0].id, "F7");
        assert!(
            selected
                .iter()
                .map(|f| f.text.chars().count())
                .sum::<usize>()
                <= 6000
        );
        assert!(selected.len() <= 4);
        let tiny = vec![Fragment {
            id: "F1".into(),
            location: "p1".into(),
            text: "áéíóú".into(),
        }];
        assert_eq!(select(&tiny, "").unwrap()[0].text, "áéíó");
    }
    #[test]
    fn local_text_extraction_preserves_original_and_reports_missing_converter() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("book.txt");
        let original = "Primer párrafo.\n\nSegundo párrafo.";
        std::fs::write(&file, original).unwrap();
        let fragments = extract(&file, None).unwrap();
        assert_eq!(fragments.len(), 2);
        assert!(fragments[1].location.contains("párrafo 2"));
        assert_eq!(std::fs::read_to_string(file).unwrap(), original);
        assert!(extract(&dir.path().join("book.pdf"), None)
            .unwrap_err()
            .contains("ebook-convert"));
    }
}
