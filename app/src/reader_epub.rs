//! Extracción de EPUB para el lector básico integrado (PROD-004).
//!
//! Parte pura, sin egui: valida el contenedor OCF, lee el OPF (spine y
//! metadatos incrustados del propio libro, conforme a PROD-011) y convierte
//! cada documento del spine en capítulos de párrafos de texto plano. La vista
//! del lector (`reader_view.rs`, por integrar) solo pagina y muestra estos
//! párrafos con las preferencias básicas de visualización.
//!
//! Alcance deliberado: sin notas, subrayados, OCR ni IA dentro del lector
//! (excluidos expresamente en PROD-004). La posición de lectura usa
//! capítulo + párrafo, estable ante cambios de tipografía o tamaño.

use std::collections::BTreeMap;
use std::io::Read;
use std::path::Path;

use quick_xml::events::{BytesStart, Event};
use quick_xml::Reader;
use serde::{Deserialize, Serialize};
use zip::ZipArchive;

/// Documento EPUB abierto y listo para leerse.
#[derive(Clone, Debug, PartialEq)]
pub struct EpubDocument {
    /// Título incrustado en el libro (OPF); vacío si el libro no lo trae.
    pub title: String,
    /// Autor incrustado en el libro (OPF); vacío si el libro no lo trae.
    pub author: String,
    /// Capítulos en el orden del spine del EPUB.
    pub chapters: Vec<Chapter>,
}

/// Un capítulo del libro: título opcional y párrafos de texto plano.
#[derive(Clone, Debug, PartialEq)]
pub struct Chapter {
    /// Título del capítulo según el propio documento; None si no declara uno.
    pub title: Option<String>,
    pub paragraphs: Vec<String>,
}

/// Posición de lectura persistible. El párrafo es estable ante cambios de
/// tipografía o tamaño de letra (PROD-004); la vista añadirá el desplazamiento
/// fino dentro del párrafo si hace falta.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReadingPosition {
    pub chapter: usize,
    pub paragraph: usize,
}

impl EpubDocument {
    /// Abre un EPUB desde bytes, validando recursos antes de extraer capítulos.
    pub fn from_bytes(data: &[u8]) -> Result<Self, String> {
        if data.len() > MAX_EPUB_BYTES {
            return Err("El EPUB supera el límite de tamaño (64 MiB)".into());
        }
        let mut archive = ZipArchive::new(std::io::Cursor::new(data))
            .map_err(|_| "El archivo ZIP está dañado o es ilegible".to_owned())?;
        if archive.len() > MAX_ZIP_ENTRIES {
            return Err("El EPUB contiene demasiadas entradas ZIP".into());
        }
        let mut unpacked = 0u64;
        for index in 0..archive.len() {
            let entry = archive.by_index(index).map_err(|e| e.to_string())?;
            unpacked = unpacked.saturating_add(entry.size());
            if unpacked > MAX_UNPACKED_BYTES {
                return Err("El EPUB supera el límite de contenido descomprimido".into());
            }
        }
        validate_ocf(&mut archive)?;
        let opf_path = container_opf_path(&mut archive)?;
        let opf = read_zip_entry(&mut archive, &opf_path, MAX_OPF_BYTES)?;
        let (title, author, spine_hrefs) = parse_opf(&opf)?;
        let base_dir = opf_base_dir(&opf_path);
        let mut chapters = Vec::new();
        let mut chapter_bytes = 0u64;
        for href in spine_hrefs {
            let zip_path = join_zip_path(&base_dir, &href)?;
            let contenido = read_zip_entry(&mut archive, &zip_path, MAX_CHAPTER_BYTES)?;
            chapter_bytes = chapter_bytes.saturating_add(contenido.len() as u64);
            if chapter_bytes > MAX_TEXT_BYTES {
                return Err("El EPUB supera el límite de texto para lectura".into());
            }
            if !es_documento_html(&contenido) {
                return Err(format!("El capítulo {zip_path} no contiene HTML legible"));
            }
            let (mut titulo_capitulo, parrafos) = html_a_parrafos(&contenido);
            if parrafos.is_empty() && titulo_capitulo.is_none() {
                // Aviso de presentación, NUNCA contenido de lectura: la capa IA
                // utiliza chapters.paragraphs como fuente de texto y citas.
                titulo_capitulo =
                    Some("Página sin texto; imagen no representada en este lector".into());
            }
            chapters.push(Chapter {
                title: titulo_capitulo,
                paragraphs: parrafos,
            });
        }
        if chapters.is_empty() {
            return Err("El EPUB no contiene capítulos legibles".to_owned());
        }
        Ok(Self {
            title,
            author,
            chapters,
        })
    }

    /// Limita también la lectura efectiva: el fichero puede crecer después
    /// de consultar sus metadatos si otro proceso lo modifica.
    pub fn from_path(path: &Path) -> Result<Self, String> {
        let file = std::fs::File::open(path)
            .map_err(|error| format!("No se pudo abrir el archivo: {error}"))?;
        if file
            .metadata()
            .map_err(|error| format!("No se pudo consultar el archivo: {error}"))?
            .len()
            > MAX_EPUB_BYTES as u64
        {
            return Err("El EPUB supera el límite de tamaño (64 MiB)".into());
        }
        let mut data = Vec::new();
        file.take(MAX_EPUB_BYTES as u64 + 1)
            .read_to_end(&mut data)
            .map_err(|error| format!("No se pudo leer el archivo: {error}"))?;
        if data.len() > MAX_EPUB_BYTES {
            return Err("El EPUB supera el límite de tamaño (64 MiB)".into());
        }
        Self::from_bytes(&data)
    }

    #[allow(dead_code)] // consumido por pruebas y futura vista de capítulos
    pub fn chapter_count(&self) -> usize {
        self.chapters.len()
    }

    /// Texto plano continuo de un capítulo (para pruebas y para la vista).
    #[allow(dead_code)]
    pub fn plain_text(&self, chapter: usize) -> Option<String> {
        self.chapters
            .get(chapter)
            .map(|chapter| chapter.paragraphs.join("\n\n"))
    }

    /// Posición por defecto al abrir un libro: principio.
    pub fn start_position() -> ReadingPosition {
        ReadingPosition {
            chapter: 0,
            paragraph: 0,
        }
    }

    /// Ajusta una posición leída de disco a los límites reales del libro
    /// (p. ej. si el archivo cambió desde la última sesión).
    pub fn clamp(&self, mut position: ReadingPosition) -> ReadingPosition {
        if self.chapters.is_empty() {
            return Self::start_position();
        }
        position.chapter = position.chapter.min(self.chapters.len() - 1);
        position.paragraph = position.paragraph.min(
            self.chapters[position.chapter]
                .paragraphs
                .len()
                .saturating_sub(1),
        );
        position
    }
}

const MAX_EPUB_BYTES: usize = 64 * 1024 * 1024;
const MAX_UNPACKED_BYTES: u64 = 128 * 1024 * 1024;
const MAX_TEXT_BYTES: u64 = 64 * 1024 * 1024;
const MAX_ZIP_ENTRIES: usize = 10_000;
const MAX_CONTAINER_BYTES: u64 = 1024 * 1024;
const MAX_OPF_BYTES: u64 = 4 * 1024 * 1024;
const MAX_CHAPTER_BYTES: u64 = 8 * 1024 * 1024;

/// Valida la firma OCF y limita los recursos XML antes de procesarlos.
fn validate_ocf<R: Read + std::io::Seek>(archive: &mut ZipArchive<R>) -> Result<(), String> {
    let mimetype = read_zip_entry(archive, "mimetype", 256)
        .map_err(|_| "No parece un EPUB: falta la entrada mimetype válida".to_owned())?;
    if mimetype.trim() != "application/epub+zip" {
        return Err("No parece un EPUB: tipo de contenedor incorrecto".to_owned());
    }
    read_zip_entry(archive, "META-INF/container.xml", MAX_CONTAINER_BYTES)
        .map_err(|error| format!("No parece un EPUB: {error}"))?;
    Ok(())
}

/// Extrae full-path del rootfile real: XML con espacios/comillas simples,
/// sin buscar substrings que puedan coincidir con comentarios u otros nodos.
fn container_opf_path<R: Read + std::io::Seek>(
    archive: &mut ZipArchive<R>,
) -> Result<String, String> {
    let container = read_zip_entry(archive, "META-INF/container.xml", MAX_CONTAINER_BYTES)?;
    let mut reader = Reader::from_reader(container.as_bytes());
    loop {
        match reader
            .read_event()
            .map_err(|e| format!("container.xml mal formado: {e}"))?
        {
            Event::Start(e) | Event::Empty(e) if e.local_name().as_ref() == b"rootfile" => {
                if let Some(path) = xml_attr(&e, b"full-path")? {
                    return join_zip_path("", &path);
                }
            }
            Event::Eof => return Err("container.xml no declara el libro raíz".into()),
            _ => {}
        }
    }
}

/// Lee texto sin superar el tamaño declarado ni el límite efectivo en bytes.
fn read_zip_entry<R: Read + std::io::Seek>(
    archive: &mut ZipArchive<R>,
    name: &str,
    limit: u64,
) -> Result<String, String> {
    let file = archive
        .by_name(name)
        .map_err(|_| format!("Entrada no encontrada en el EPUB: {name}"))?;
    if file.size() > limit {
        return Err(format!("Recurso EPUB demasiado grande: {name}"));
    }
    let mut bytes = Vec::new();
    file.take(limit + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| format!("No se pudo leer {name}: {e}"))?;
    if bytes.len() as u64 > limit {
        return Err(format!("Recurso EPUB excede el límite: {name}"));
    }
    String::from_utf8(bytes)
        .map(|s| s.trim_start_matches('\u{feff}').to_owned())
        .map_err(|_| format!("Recurso EPUB no es UTF-8: {name}"))
}

fn xml_attr(element: &BytesStart<'_>, name: &[u8]) -> Result<Option<String>, String> {
    for attribute in element.attributes() {
        let attribute = attribute.map_err(|e| format!("Atributo XML incorrecto: {e}"))?;
        if attribute.key.local_name().as_ref() == name {
            return attribute
                .normalized_value(quick_xml::XmlVersion::Implicit1_0)
                .map(|v| Some(v.into_owned()))
                .map_err(|e| e.to_string());
        }
    }
    Ok(None)
}

fn opf_base_dir(opf_path: &str) -> String {
    opf_path
        .rsplit_once('/')
        .map(|(base, _)| base.to_owned())
        .unwrap_or_default()
}

/// Resuelve un URI EPUB contra su carpeta OPF, sin permitir salir de la raíz
/// ZIP ni confundir un href codificado con una ruta del sistema operativo.
fn join_zip_path(base: &str, href: &str) -> Result<String, String> {
    let href = href.split('#').next().unwrap_or(href);
    let href = href.split('?').next().unwrap_or(href);
    if href.is_empty() {
        return Err("Ruta EPUB vacía".into());
    }
    let mut decoded = Vec::with_capacity(href.len());
    let mut bytes = href.bytes();
    while let Some(byte) = bytes.next() {
        if byte == b'%' {
            let high = bytes
                .next()
                .and_then(|v| (v as char).to_digit(16))
                .ok_or("Codificación de URI EPUB inválida")?;
            let low = bytes
                .next()
                .and_then(|v| (v as char).to_digit(16))
                .ok_or("Codificación de URI EPUB inválida")?;
            decoded.push((high * 16 + low) as u8);
        } else {
            decoded.push(byte);
        }
    }
    let href = String::from_utf8(decoded).map_err(|_| "URI EPUB no es UTF-8")?;
    if href.starts_with('/') || href.contains(['\\', ':', '\0']) {
        return Err("Ruta EPUB fuera del contenedor".into());
    }
    let mut parts: Vec<&str> = base.split('/').filter(|p| !p.is_empty()).collect();
    for part in href.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                parts.pop().ok_or("Ruta EPUB fuera del contenedor")?;
            }
            _ => parts.push(part),
        }
    }
    if parts.is_empty() {
        return Err("Ruta EPUB vacía".into());
    }
    Ok(parts.join("/"))
}

/// Los metadatos provienen del OPF; IDs y orden proceden de manifest/spine.
/// Se reconocen namespaces por local_name y atributos XML reales.
fn parse_opf(opf: &str) -> Result<(String, String, Vec<String>), String> {
    let mut reader = Reader::from_reader(opf.as_bytes());
    let mut title = String::new();
    let mut author = String::new();
    let mut current: Option<(Vec<u8>, String)> = None;
    let mut in_metadata = false;
    let mut in_manifest = false;
    let mut in_spine = false;
    let mut has_manifest = false;
    let mut has_spine = false;
    let mut hrefs: BTreeMap<String, (String, bool)> = BTreeMap::new();
    let mut spine_ids = Vec::new();
    loop {
        let event = reader
            .read_event()
            .map_err(|e| format!("OPF mal formado: {e}"))?;
        let empty = matches!(&event, Event::Empty(_));
        match event {
            Event::Start(e) | Event::Empty(e) => {
                let name = e.local_name().as_ref().to_vec();
                match name.as_slice() {
                    b"metadata" if !empty => in_metadata = true,
                    b"manifest" if !empty => {
                        in_manifest = true;
                        has_manifest = true;
                    }
                    b"spine" if !empty => {
                        in_spine = true;
                        has_spine = true;
                    }
                    b"title" | b"creator" if in_metadata && !empty && current.is_none() => {
                        current = Some((name.clone(), String::new()));
                    }
                    b"item" if in_manifest => {
                        if let (Some(id), Some(href)) =
                            (xml_attr(&e, b"id")?, xml_attr(&e, b"href")?)
                        {
                            let media = xml_attr(&e, b"media-type")?
                                .unwrap_or_default()
                                .to_ascii_lowercase();
                            let path = href.split('#').next().unwrap_or(&href).to_ascii_lowercase();
                            let html = media.contains("html")
                                || path.ends_with(".xhtml")
                                || path.ends_with(".html");
                            if hrefs.insert(id.clone(), (href, html)).is_some() {
                                return Err(format!("ID duplicado en el manifest: {id}"));
                            }
                        }
                    }
                    b"itemref" if in_spine => {
                        if let Some(idref) = xml_attr(&e, b"idref")? {
                            if xml_attr(&e, b"linear")?.as_deref() != Some("no") {
                                spine_ids.push(idref);
                            }
                        } else {
                            return Err("El spine contiene itemref sin idref".into());
                        }
                    }
                    _ => {}
                }
            }
            Event::Text(e) => {
                if let Some((_, text)) = &mut current {
                    let decoded = e.decode().map_err(|e| e.to_string())?;
                    text.push_str(&quick_xml::escape::unescape(&decoded).map_err(|e| e.to_string())?);
                }
            }
            Event::CData(e) => {
                if let Some((_, text)) = &mut current {
                    text.push_str(&e.decode().map_err(|e| e.to_string())?);
                }
            }
            Event::GeneralRef(e) => {
                if let Some((_, text)) = &mut current {
                    let entity = format!("&{};", e.decode().map_err(|e| e.to_string())?);
                    text.push_str(
                        &quick_xml::escape::unescape(&entity).map_err(|e| e.to_string())?,
                    );
                }
            }
            Event::End(e) => {
                let name = e.local_name().as_ref().to_vec();
                if current.as_ref().is_some_and(|(tag, _)| tag == &name) {
                    let (tag, value) = current.take().expect("tag estaba activo");
                    let value = value.trim().to_owned();
                    if tag.as_slice() == b"title" && title.is_empty() {
                        title = value;
                    } else if tag.as_slice() == b"creator" && author.is_empty() {
                        author = value;
                    }
                }
                match name.as_slice() {
                    b"metadata" => in_metadata = false,
                    b"manifest" => in_manifest = false,
                    b"spine" => in_spine = false,
                    _ => {}
                }
            }
            Event::Eof => break,
            _ => {}
        }
    }
    if !has_manifest {
        return Err("El OPF no tiene manifest".into());
    }
    if !has_spine {
        return Err("El OPF no tiene spine".into());
    }
    let mut order = Vec::new();
    for idref in spine_ids {
        let (href, is_html) = hrefs
            .get(&idref)
            .ok_or_else(|| format!("El spine referencia un ID ausente: {idref}"))?;
        if *is_html {
            order.push(href.clone());
        }
    }
    if order.is_empty() {
        return Err("El spine del EPUB no referencia documentos legibles".into());
    }
    Ok((title, author, order))
}

/// Un documento del spine es HTML si declara DOCTYPE/html/body o una
/// extensión xhtml/html aunque el media-type falte en EPUB antiguos.
fn es_documento_html(contenido: &str) -> bool {
    let inicio: String = contenido.chars().take(2000).collect();
    let minusculas = inicio.to_lowercase();
    minusculas.contains("<html") || minusculas.contains("<body") || minusculas.contains("<!doctype")
}

/// Convierte un documento XHTML/HTML en (título, párrafos): los títulos
/// vienen de <h1>…<h6>; los cortes de párrafo en </p>, <br>, </div>, </li>
/// y encabezados. Ignora el contenido de <style> y <script>.
fn html_a_parrafos(html: &str) -> (Option<String>, Vec<String>) {
    let sin_estilos = quitar_secciones(html, "<style", "</style>");
    let sin_scripts = quitar_secciones(&sin_estilos, "<script", "</script>");
    // El título del capítulo: preferente del primer <h1>…<h3>; si no hay,
    // el <title> de la cabecera. El texto consumido como título no se
    // duplica como párrafo.
    let mut titulo_encabezado: Option<String> = None;
    let mut titulo_head: Option<String> = None;
    let mut parrafos: Vec<String> = Vec::new();
    let mut actual = String::new();
    let mut bytes = sin_scripts.chars().peekable();
    while let Some(ch) = bytes.next() {
        if ch == '<' {
            let mut etiqueta = String::new();
            for c in bytes.by_ref() {
                if c == '>' {
                    break;
                }
                etiqueta.push(c);
            }
            let minusculas = etiqueta.to_lowercase();
            let nombre = minusculas
                .trim_start_matches('/')
                .split(|c: char| c.is_whitespace())
                .next()
                .unwrap_or_default()
                .to_owned();
            let cierra = minusculas.starts_with('/');
            if nombre == "title" && !cierra && titulo_head.is_none() {
                let mut contenido = String::new();
                // Con peek(): el «<» del cierre queda para el bucle principal.
                while let Some(&c) = bytes.peek() {
                    if c == '<' {
                        break;
                    }
                    bytes.next();
                    contenido.push(c);
                }
                let limpio = decodificar_entidades(contenido.trim());
                if !limpio.is_empty() {
                    titulo_head = Some(limpio);
                }
                continue;
            }
            let corta = matches!(
                nombre.as_str(),
                "p" | "div" | "li" | "tr" | "br" | "h1" | "h2" | "h3" | "h4" | "h5" | "h6"
            );
            if corta {
                let limpio = decodificar_entidades(actual.trim());
                if !limpio.is_empty() {
                    parrafos.push(limpio);
                }
                actual.clear();
                // El primer encabezado del cuerpo da título al capítulo.
                if titulo_encabezado.is_none()
                    && matches!(nombre.as_str(), "h1" | "h2" | "h3")
                    && !cierra
                {
                    let mut contenido = String::new();
                    // Con peek(): el «<» del cierre no se consume y el bucle
                    // principal procesa la etiqueta de cierre con normalidad.
                    while let Some(&c) = bytes.peek() {
                        if c == '<' {
                            break;
                        }
                        bytes.next();
                        contenido.push(c);
                    }
                    let limpio = decodificar_entidades(contenido.trim());
                    if !limpio.is_empty() {
                        titulo_encabezado = Some(limpio);
                    }
                    continue;
                }
            }
            continue;
        }
        actual.push(ch);
    }
    let limpio = decodificar_entidades(actual.trim());
    if !limpio.is_empty() {
        parrafos.push(limpio);
    }
    (titulo_encabezado.or(titulo_head), parrafos)
}

/// Elimina secciones completas (etiqueta de apertura a cierre) de forma
/// tolerante, usada para <style> y <script>.
fn quitar_secciones(texto: &str, apertura: &str, cierre: &str) -> String {
    let minusculas_apertura = apertura.to_lowercase();
    let minusculas_cierre = cierre.to_lowercase();
    let mut resultado = String::with_capacity(texto.len());
    let mut restante = texto;
    loop {
        let minusculas = restante.to_lowercase();
        let Some(inicio) = minusculas.find(&minusculas_apertura) else {
            resultado.push_str(restante);
            break;
        };
        resultado.push_str(&restante[..inicio]);
        let desde_apertura = &restante[inicio..];
        let minusculas_restantes = desde_apertura.to_lowercase();
        match minusculas_restantes.find(&minusculas_cierre) {
            Some(fin) => {
                restante = &desde_apertura[fin + cierre.len()..];
            }
            None => break, // sin cierre: se descarta el resto
        }
    }
    resultado
}

/// Decodifica las entidades HTML habituales y las numéricas; las desconocidas
/// se dejan tal cual (nunca se inventa contenido). Es texto plano para
/// mostrar: no se re-escapa nada.
fn decodificar_entidades(texto: &str) -> String {
    let comunes = [
        ("&lt;", "<"),
        ("&gt;", ">"),
        ("&quot;", "\""),
        ("&apos;", "'"),
        ("&nbsp;", "\u{00a0}"),
        ("&mdash;", "\u{2014}"),
        ("&ndash;", "\u{2013}"),
        ("&hellip;", "\u{2026}"),
        ("&laquo;", "\u{ab}"),
        ("&raquo;", "\u{bb}"),
        ("&amp;", "&"), // la última: así &amp; no re-descodifica &lt; etc.
    ];
    let mut resultado = texto.to_owned();
    for (entidad, valor) in comunes {
        resultado = resultado.replace(entidad, valor);
    }
    // Sustituye solo entidades numéricas terminadas por ';': un número
    // incompleto o fuera de rango se conserva literal, sin panic ni truncado.
    let mut decoded = String::with_capacity(resultado.len());
    let mut remaining = resultado.as_str();
    while let Some(start) = remaining.find("&#") {
        decoded.push_str(&remaining[..start]);
        let after = &remaining[start + 2..];
        let (radix, offset, digits_start) =
            if let Some(hex) = after.strip_prefix('x').or_else(|| after.strip_prefix('X')) {
                (16, 1, hex)
            } else {
                (10, 0, after)
            };
        let digits: String = digits_start
            .chars()
            .take_while(|c| if radix == 16 { c.is_ascii_hexdigit() } else { c.is_ascii_digit() })
            .collect();
        let marker = offset + digits.len();
        let replacement = if !digits.is_empty() && after.as_bytes().get(marker) == Some(&b';') {
            u32::from_str_radix(&digits, radix).ok().and_then(char::from_u32)
        } else {
            None
        };
        if let Some(ch) = replacement {
            decoded.push(ch);
            remaining = &after[marker + 1..];
        } else {
            decoded.push_str("&#");
            remaining = after;
        }
    }
    decoded.push_str(remaining);
    decoded
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use std::io::Write;

    /// EPUB mínimo válido generado en memoria: mimetype (sin compresión,
    /// primera entrada), container.xml, OPF con dos capítulos.
    pub(crate) fn epub_de_prueba() -> Vec<u8> {
        let mut buffer = std::io::Cursor::new(Vec::new());
        {
            let mut zip = zip::ZipWriter::new(&mut buffer);
            let opciones: zip::write::FileOptions<'_, ()> = zip::write::FileOptions::default()
                .compression_method(zip::CompressionMethod::Stored);
            zip.start_file("mimetype", opciones).unwrap();
            zip.write_all(b"application/epub+zip").unwrap();
            let opciones_deflate: zip::write::FileOptions<'_, ()> =
                zip::write::FileOptions::default()
                    .compression_method(zip::CompressionMethod::Deflated);
            zip.start_file("META-INF/container.xml", opciones_deflate)
                .unwrap();
            zip.write_all(
                r#"<?xml version="1.0"?>
<container version="1.0" xmlns="urn:oasis:names:tc:opendocument:xmlns:container">
  <rootfiles><rootfile full-path="OEBPS/content.opf" media-type="application/oebps-package+xml"/></rootfiles>
</container>"#.as_bytes(),
            )
            .unwrap();
            zip.start_file("OEBPS/content.opf", opciones_deflate)
                .unwrap();
            zip.write_all(
                r#"<?xml version="1.0"?>
<package xmlns="http://www.idpf.org/2007/opf" version="2.0">
  <metadata xmlns:dc="http://purl.org/dc/elements/1.1/">
    <dc:title>Doña Luz</dc:title>
    <dc:creator>Author &amp; Compañía</dc:creator>
  </metadata>
  <manifest>
    <item id="c1" href="cap1.xhtml" media-type="application/xhtml+xml"/>
    <item id="c2" href="sub/cap2.xhtml" media-type="application/xhtml+xml"/>
    <item id="css" href="estilos.css" media-type="text/css"/>
  </manifest>
  <spine><itemref idref="c1"/><itemref idref="c2"/></spine>
</package>"#
                    .as_bytes(),
            )
            .unwrap();
            zip.start_file("OEBPS/cap1.xhtml", opciones_deflate)
                .unwrap();
            zip.write_all(
                r#"<html><head><title>Capítulo uno</title></head>
<body><h1>Capítulo uno</h1>
<p>Primer párrafo con &amp; entidades &lt;reales&gt;.</p>
<p>Segundo párrafo.</p></body></html>"#
                    .as_bytes(),
            )
            .unwrap();
            zip.start_file("OEBPS/sub/cap2.xhtml", opciones_deflate)
                .unwrap();
            zip.write_all(
                r#"<html><body>
<div><p>Capítulo dos sin encabezado, título desde &laquo;title&raquo;.</p></div>
<script>esto no debe aparecer</script>
<style>pb { display: none }</style>
<p>Último párrafo.</p></body></html>"#
                    .as_bytes(),
            )
            .unwrap();
            zip.finish().unwrap();
        }
        buffer.into_inner()
    }

    #[test]
    fn abre_epub_valido_con_metadatos_spine_y_capitulos() {
        let libro = EpubDocument::from_bytes(&epub_de_prueba()).expect("EPUB de prueba debe abrir");
        assert_eq!(libro.title, "Doña Luz");
        assert_eq!(
            libro.author, "Author & Compañía",
            "entidades del OPF decodificadas"
        );
        assert_eq!(libro.chapter_count(), 2);
        assert_eq!(
            libro.chapters[0].title.as_deref(),
            Some("Capítulo uno"),
            "del encabezado h1"
        );
        assert_eq!(
            libro.chapters[0].paragraphs,
            vec![
                "Primer párrafo con & entidades <reales>.",
                "Segundo párrafo.",
            ]
        );
        // href relativo «sub/cap2.xhtml» resuelto desde la carpeta del OPF.
        assert_eq!(libro.chapters[1].paragraphs.len(), 2);
        assert!(libro.chapters[1].paragraphs[0].contains("«title»"));
        assert!(!libro.plain_text(1).unwrap().contains("no debe aparecer"));
    }

    #[test]
    fn rechaza_archivos_que_no_son_epub_sin_inventar_contenido() {
        // ZIP sin mimetype EPUB.
        let mut buffer = std::io::Cursor::new(Vec::new());
        {
            let mut zip = zip::ZipWriter::new(&mut buffer);
            let opciones: zip::write::FileOptions<'_, ()> = zip::write::FileOptions::default()
                .compression_method(zip::CompressionMethod::Stored);
            zip.start_file("mimetype", opciones).unwrap();
            zip.write_all(b"application/pdf").unwrap();
            zip.finish().unwrap();
        }
        let error = EpubDocument::from_bytes(&buffer.into_inner()).unwrap_err();
        assert!(error.contains("No parece un EPUB"));
        // Bytes que ni siquiera son ZIP.
        let error = EpubDocument::from_bytes(b"esto no es un zip").unwrap_err();
        assert!(error.contains("ZIP"));
    }

    #[test]
    fn la_posicion_inicial_y_el_ajuste_a_limites_son_correctos() {
        let libro = EpubDocument::from_bytes(&epub_de_prueba()).unwrap();
        // La navegación por páginas vive en reader_view; aquí se valida el
        // punto de partida y el ajuste de posiciones guardadas.
        assert_eq!(
            EpubDocument::start_position(),
            ReadingPosition {
                chapter: 0,
                paragraph: 0
            }
        );
        // Posición imposible (archivo cambiado) se ajusta sin caer fuera.
        let ajustada = libro.clamp(ReadingPosition {
            chapter: 9,
            paragraph: 99,
        });
        assert_eq!(
            ajustada,
            ReadingPosition {
                chapter: 1,
                paragraph: 1
            }
        );
    }

    #[test]
    fn entidades_desconocidas_se_conservan_literalmente() {
        assert_eq!(
            decodificar_entidades("A &amp; B &inventada; C &#65; D &#x42;"),
            "A & B &inventada; C A D B"
        );
        // Fuera de rango para un carácter válido: se conserva literal.
        assert_eq!(decodificar_entidades("&#999999999;"), "&#999999999;");
        assert_eq!(decodificar_entidades("&#65"), "&#65");
        assert_eq!(decodificar_entidades("&#x41"), "&#x41");
        assert_eq!(decodificar_entidades("A &#65x B &#66; C"), "A &#65x B B C");
        assert_eq!(decodificar_entidades("&#x1F600;"), "😀");
    }

    #[test]
    fn rutas_zip_se_resuelven_con_segmentos_relativos() {
        assert_eq!(join_zip_path("OEBPS", "cap1.xhtml").unwrap(), "OEBPS/cap1.xhtml");
        assert_eq!(
            join_zip_path("OEBPS", "sub/cap2.xhtml").unwrap(),
            "OEBPS/sub/cap2.xhtml"
        );
        assert_eq!(
            join_zip_path("OEBPS/textos", "../estilos.css").unwrap(),
            "OEBPS/estilos.css"
        );
        assert_eq!(join_zip_path("", "raiz.xhtml").unwrap(), "raiz.xhtml");
    }

    /// Construye fixtures sin depender de rutas locales ni de Calibre.
    fn fixture(container: &str, opf_path: &str, opf: &str, chapters: &[(&str, &str)]) -> Vec<u8> {
        let mut buffer = std::io::Cursor::new(Vec::new());
        {
            let mut zip = zip::ZipWriter::new(&mut buffer);
            let stored: zip::write::FileOptions<'_, ()> =
                zip::write::FileOptions::default().compression_method(zip::CompressionMethod::Stored);
            let deflated: zip::write::FileOptions<'_, ()> =
                zip::write::FileOptions::default().compression_method(zip::CompressionMethod::Deflated);
            zip.start_file("mimetype", stored).unwrap();
            zip.write_all(b"application/epub+zip").unwrap();
            zip.start_file("META-INF/container.xml", deflated).unwrap();
            zip.write_all(container.as_bytes()).unwrap();
            zip.start_file(opf_path, deflated).unwrap();
            zip.write_all(opf.as_bytes()).unwrap();
            for (path, text) in chapters {
                zip.start_file(*path, deflated).unwrap();
                zip.write_all(text.as_bytes()).unwrap();
            }
            zip.finish().unwrap();
        }
        buffer.into_inner()
    }

    #[test]
    fn xml_con_comillas_simples_espacios_namespaces_uris_y_orden_spine() {
        let container = r#"<?xml version='1.0'?><container>
        <!-- full-path="no-existe.opf" -->
        <rootfiles><rootfile media-type='application/oebps-package+xml'
            full-path = 'OPS/mi%20libro.opf'/></rootfiles></container>"#;
        let opf = r#"<?xml version='1.0'?><package xmlns:dct='http://purl.org/dc/elements/1.1/'>
          <metadata><dct:title>Libro &amp; Compañía</dct:title><dct:creator>Autora</dct:creator></metadata>
          <manifest><item id = 'uno' href = 'texto%20uno.xhtml' media-type = 'application/xhtml+xml'/>
          <item id='dos' href='sub/../dos.xhtml' media-type='application/xhtml+xml'/></manifest>
          <spine><itemref idref = 'dos'/><itemref idref = 'uno'/></spine></package>"#;
        let file = fixture(
            container, "OPS/mi libro.opf", opf,
            &[
                ("OPS/texto uno.xhtml", "<html><body><p>Primero</p></body></html>"),
                ("OPS/dos.xhtml", "<html><body><p>Segundo</p></body></html>"),
            ],
        );
        let book = EpubDocument::from_bytes(&file).unwrap();
        assert_eq!(book.title, "Libro & Compañía");
        assert_eq!(book.author, "Autora");
        assert_eq!(book.chapters.len(), 2);
        assert_eq!(book.chapters[0].paragraphs, vec!["Segundo"]);
        assert_eq!(book.chapters[1].paragraphs, vec!["Primero"]);
    }

    #[test]
    fn no_oculta_capitulos_ausentes_ni_spine_roto() {
        let container = "<container><rootfiles><rootfile full-path='OPS/content.opf'/></rootfiles></container>";
        let opf = "<package><manifest><item id='uno' href='falta.xhtml' media-type='application/xhtml+xml'/></manifest><spine><itemref idref='uno'/></spine></package>";
        let file = fixture(container, "OPS/content.opf", opf, &[]);
        assert!(EpubDocument::from_bytes(&file).unwrap_err().contains("Entrada no encontrada"));
        let broken = opf.replace("idref='uno'", "idref='inexistente'");
        let file = fixture(container, "OPS/content.opf", &broken, &[]);
        assert!(EpubDocument::from_bytes(&file).unwrap_err().contains("ID ausente"));
    }

    #[test]
    fn rechaza_uris_que_salen_del_zip_o_tienen_codificacion_invalida() {
        assert_eq!(
            join_zip_path("OPS/sub", "../cap%20uno.xhtml").unwrap(),
            "OPS/cap uno.xhtml"
        );
        for bad in [
            "../../../fuera.xhtml", "%2e%2e/%2e%2e/fuera.xhtml", "/absolute.xhtml",
            "C:%5cWindows%5cfile", "http://externo/libro.xhtml", "%5cshare",
            "foo%00bar", "bad%XZpath.xhtml", "bad%.xhtml",
        ] {
            assert!(join_zip_path("OPS", bad).is_err(), "se aceptó {bad}");
        }
    }

    #[test]
    fn limita_recursos_descomprimidos_antes_de_analizarlos() {
        let container = "<container><rootfiles><rootfile full-path='OPS/content.opf'/></rootfiles></container>";
        let huge = "a".repeat(MAX_OPF_BYTES as usize + 1);
        let file = fixture(container, "OPS/content.opf", &huge, &[]);
        assert!(EpubDocument::from_bytes(&file).unwrap_err().contains("demasiado grande"));
    }


    #[test]
    fn imagen_sin_texto_conserva_su_posicion_y_no_oculta_el_capitulo_siguiente() {
        let container = "<container><rootfiles><rootfile full-path='OPS/content.opf'/></rootfiles></container>";
        let opf = "<package><manifest>\
        <item id='cover' href='portada.xhtml' media-type='application/xhtml+xml'/>\
        <item id='content' href='texto.xhtml' media-type='application/xhtml+xml'/>\
        </manifest><spine><itemref idref='cover'/><itemref idref='content'/></spine></package>";
        let epub = fixture(
            container,
            "OPS/content.opf",
            opf,
            &[
                ("OPS/portada.xhtml", "<html><body><img src='cover.jpg'/></body></html>"),
                ("OPS/texto.xhtml", "<html><body><p>Capítulo legible.</p></body></html>"),
            ],
        );
        let book = EpubDocument::from_bytes(&epub).unwrap();
        assert_eq!(book.chapters.len(), 2, "no se omiten capítulos del spine");
        assert_eq!(
            book.chapters[0].title.as_deref(),
            Some("Página sin texto; imagen no representada en este lector")
        );
        assert!(book.chapters[0].paragraphs.is_empty());
        // La extracción de texto/IA consume solo paragraphs: el aviso visual
        // no se ofrece como una cita ficticia ni se atribuye al EPUB.
        let extracted = book.chapters.iter()
            .flat_map(|chapter| chapter.paragraphs.iter())
            .cloned()
            .collect::<Vec<_>>()
            .join("\n");
        assert!(!extracted.contains("Página sin texto"));
        assert_eq!(book.chapters[1].paragraphs, vec!["Capítulo legible."]);
    }

    #[test]
    fn from_path_rechaza_archivos_mas_grandes_antes_de_cargarlos() {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        tmp.as_file().set_len(MAX_EPUB_BYTES as u64 + 1).unwrap();
        assert!(EpubDocument::from_path(tmp.path())
            .unwrap_err()
            .contains("supera el límite"));
    }

}
