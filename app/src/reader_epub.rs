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

use std::io::Read;
use std::path::Path;

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
    /// Abre un EPUB desde bytes. Valida mimetype y container.xml antes de
    /// aceptar el archivo: una extensión renombrada no es un EPUB válido.
    pub fn from_bytes(data: &[u8]) -> Result<Self, String> {
        let cursor = std::io::Cursor::new(data);
        let mut archive = ZipArchive::new(cursor)
            .map_err(|_| "El archivo ZIP está dañado o es ilegible".to_owned())?;
        validate_ocf(&mut archive)?;
        let opf_path = container_opf_path(&mut archive)?;
        let opf = read_zip_entry(&mut archive, &opf_path)?;
        let (title, author, spine_hrefs) = parse_opf(&opf)?;
        let base_dir = opf_base_dir(&opf_path);
        let mut chapters = Vec::new();
        for href in spine_hrefs {
            let zip_path = join_zip_path(&base_dir, &href);
            let contenido = match read_zip_entry(&mut archive, &zip_path) {
                Ok(contenido) => contenido,
                Err(_) => continue, // spine apunta a un recurso ausente: se salta
            };
            let es_html = es_documento_html(&contenido);
            if !es_html {
                continue;
            }
            let (titulo_capitulo, parrafos) = html_a_parrafos(&contenido);
            if parrafos.is_empty() && titulo_capitulo.is_none() {
                continue;
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

    /// Abre un EPUB desde una ruta de archivo.
    pub fn from_path(path: &Path) -> Result<Self, String> {
        let data =
            std::fs::read(path).map_err(|error| format!("No se pudo leer el archivo: {error}"))?;
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

/// Contenedor OCF válido: entrada «mimetype» con el tipo exacto y
/// META-INF/container.xml presente. Es la misma regla que aplica la
/// transferencia (transfer.rs): la firma PK sola no basta.
fn validate_ocf<R: Read + std::io::Seek>(archive: &mut ZipArchive<R>) -> Result<(), String> {
    let mimetype = read_zip_entry(archive, "mimetype")
        .map_err(|_| "No parece un EPUB: falta la entrada mimetype".to_owned())?;
    if mimetype.trim() != "application/epub+zip" {
        return Err(
            "No parece un EPUB: el tipo de contenedor no es application/epub+zip".to_owned(),
        );
    }
    read_zip_entry(archive, "META-INF/container.xml")
        .map_err(|_| "No parece un EPUB: falta META-INF/container.xml".to_owned())?;
    Ok(())
}

/// Ruta del OPF declarada en META-INF/container.xml.
fn container_opf_path<R: Read + std::io::Seek>(
    archive: &mut ZipArchive<R>,
) -> Result<String, String> {
    let container = read_zip_entry(archive, "META-INF/container.xml")?;
    let marcador = "full-path=\"";
    let inicio = container
        .find(marcador)
        .map(|pos| pos + marcador.len())
        .ok_or("container.xml no declara el libro raíz")?;
    let fin = container[inicio..]
        .find('"')
        .map(|offset| inicio + offset)
        .ok_or("container.xml mal formado")?;
    let ruta = container[inicio..fin].trim().to_owned();
    if ruta.is_empty() {
        return Err("container.xml declara una ruta vacía".to_owned());
    }
    Ok(ruta)
}

/// Lee un archivo del ZIP como texto UTF-8 (con BOM tolerado).
fn read_zip_entry<R: Read + std::io::Seek>(
    archive: &mut ZipArchive<R>,
    name: &str,
) -> Result<String, String> {
    let mut file = archive
        .by_name(name)
        .map_err(|_| format!("Entrada no encontrada en el EPUB: {name}"))?;
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes)
        .map_err(|error| format!("No se pudo leer {name}: {error}"))?;
    let texto = String::from_utf8_lossy(&bytes);
    Ok(texto.trim_start_matches('\u{feff}').to_owned())
}

/// Carpeta base del OPF dentro del ZIP («OEBPS», etc.); vacía si está en raíz.
fn opf_base_dir(opf_path: &str) -> String {
    match opf_path.rfind('/') {
        Some(pos) => opf_path[..pos].to_owned(),
        None => String::new(),
    }
}

/// Une la carpeta base del OPF con un href relativo del manifest, usando
/// separadores «/» como en el contenedor ZIP.
fn join_zip_path(base: &str, href: &str) -> String {
    let ruta = format!("{base}/{href}");
    let mut segmentos: Vec<&str> = Vec::new();
    for parte in ruta.split('/') {
        match parte {
            "" | "." => {}
            ".." => {
                segmentos.pop();
            }
            otro => segmentos.push(otro),
        }
    }
    segmentos.join("/")
}

/// Extrae (título, autor, hrefs del spine) del OPF con escaneo ligero, sin
/// dependencias nuevas. Los metadatos son los incrustados del libro (PROD-011).
fn parse_opf(opf: &str) -> Result<(String, String, Vec<String>), String> {
    // Los metadatos se decodifican: los OPF suelen traer entidades XML.
    let title =
        decodificar_entidades(&texto_entre(opf, "<dc:title", "</dc:title>").unwrap_or_default());
    let author = decodificar_entidades(
        &texto_entre(opf, "<dc:creator", "</dc:creator>").unwrap_or_default(),
    );
    // Manifest: id → href y media-type.
    let mut hrefs: Vec<(String, String, bool)> = Vec::new(); // (id, href, es_html)
    let manifest =
        texto_entre(opf, "<manifest", "</manifest>").ok_or("El OPF no tiene manifest")?;
    for item in segmentos_xml(&manifest, "<item") {
        let id = atributo(&item, "id").unwrap_or_default();
        let href = atributo(&item, "href").unwrap_or_default();
        let media = atributo(&item, "media-type").unwrap_or_default();
        let es_html = media.contains("html")
            || href.to_lowercase().ends_with(".xhtml")
            || href.to_lowercase().ends_with(".html");
        if !id.is_empty() && !href.is_empty() {
            hrefs.push((id, href, es_html));
        }
    }
    // Spine: orden de lectura.
    let spine = texto_entre(opf, "<spine", "</spine>").ok_or("El OPF no tiene spine")?;
    let mut orden = Vec::new();
    for itemref in segmentos_xml(&spine, "<itemref") {
        if let Some(idref) = atributo(&itemref, "idref") {
            if let Some((_, href, es_html)) = hrefs.iter().find(|(id, _, _)| *id == idref) {
                if *es_html {
                    orden.push(href.clone());
                }
            }
        }
    }
    if orden.is_empty() {
        return Err("El spine del EPUB no referencia documentos legibles".to_owned());
    }
    Ok((title.trim().to_owned(), author.trim().to_owned(), orden))
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

/// Fragmentos del texto entre apariciones de una etiqueta de apertura.
fn segmentos_xml(texto: &str, etiqueta: &str) -> Vec<String> {
    let mut resultados = Vec::new();
    let mut restante = texto;
    while let Some(pos) = restante.find(etiqueta) {
        let desde = &restante[pos + etiqueta.len()..];
        match desde.find('>') {
            Some(fin) => {
                resultados.push(desde[..fin].to_owned());
                restante = &desde[fin + 1..];
            }
            None => break,
        }
    }
    resultados
}

/// Texto entre una etiqueta de apertura y su cierre; tolera atributos.
fn texto_entre(texto: &str, apertura: &str, cierre: &str) -> Option<String> {
    let inicio = texto.find(apertura)? + apertura.len();
    let desde = &texto[inicio..];
    let relativo = desde.find('>')? + 1;
    let desde_contenido = &desde[relativo..];
    let fin = desde_contenido.find(cierre)?;
    Some(desde_contenido[..fin].trim().to_owned())
}

/// Valor de un atributo dentro de la porción de etiqueta indicada.
fn atributo(etiqueta: &str, nombre: &str) -> Option<String> {
    let patron_comillas = format!("{nombre}=\"");
    if let Some(pos) = etiqueta.find(&patron_comillas) {
        let desde = &etiqueta[pos + patron_comillas.len()..];
        if let Some(fin) = desde.find('"') {
            return Some(desde[..fin].to_owned());
        }
    }
    let patron_simple = format!("{nombre}=");
    let pos = etiqueta.find(&patron_simple)?;
    let desde = &etiqueta[pos + patron_simple.len()..];
    let fin = desde
        .find(|c: char| c.is_whitespace())
        .unwrap_or(desde.len());
    Some(desde[..fin].trim_matches('\'').to_owned())
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
    // Numéricas decimales y hexadecimales.
    while let Some(pos) = resultado.find("&#") {
        let desde = &resultado[pos + 2..];
        let (valor, consumo) =
            if let Some(hex) = desde.strip_prefix('x').or_else(|| desde.strip_prefix('X')) {
                let digits: String = hex.chars().take_while(|c| c.is_ascii_hexdigit()).collect();
                (u32::from_str_radix(&digits, 16).ok(), 1 + digits.len())
            } else {
                let digits: String = desde.chars().take_while(|c| c.is_ascii_digit()).collect();
                (digits.parse::<u32>().ok(), digits.len())
            };
        match valor.and_then(char::from_u32) {
            Some(ch) if consumo > 0 => {
                resultado.replace_range(pos..pos + 2 + consumo + 1, &ch.to_string());
            }
            _ => break, // entidad desconocida o mal formada: se deja literal
        }
    }
    resultado
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
    }

    #[test]
    fn rutas_zip_se_resuelven_con_segmentos_relativos() {
        assert_eq!(join_zip_path("OEBPS", "cap1.xhtml"), "OEBPS/cap1.xhtml");
        assert_eq!(
            join_zip_path("OEBPS", "sub/cap2.xhtml"),
            "OEBPS/sub/cap2.xhtml"
        );
        assert_eq!(
            join_zip_path("OEBPS/textos", "../estilos.css"),
            "OEBPS/estilos.css"
        );
        assert_eq!(join_zip_path("", "raiz.xhtml"), "raiz.xhtml");
    }
}
