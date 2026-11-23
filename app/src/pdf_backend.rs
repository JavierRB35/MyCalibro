//! Windows' built-in PDF renderer. No bundled PDF engine or external DLL is required.
use std::path::Path;

pub struct RenderedPdf {
    pub page_index: u32,
    pub page_count: u32,
    pub png: Vec<u8>,
}

/// Call on a fresh worker thread, never on the UI thread.
#[cfg(windows)]
pub fn render_page(path: &Path, page_index: u32, width: u32) -> Result<RenderedPdf, String> {
    use windows::{
        core::HSTRING,
        Data::Pdf::{PdfDocument, PdfPageRenderOptions},
        Storage::{
            StorageFile,
            Streams::{DataReader, InMemoryRandomAccessStream},
        },
        Win32::System::WinRT::{RoInitialize, RoUninitialize, RO_INIT_MULTITHREADED},
    };
    struct Apartment;
    impl Drop for Apartment {
        fn drop(&mut self) {
            unsafe { RoUninitialize() };
        }
    }
    unsafe { RoInitialize(RO_INIT_MULTITHREADED) }
        .map_err(|e| format!("No se pudo iniciar el lector PDF de Windows: {e}"))?;
    let _apartment = Apartment;
    let path = std::fs::canonicalize(path).map_err(|e| e.to_string())?;
    let path = path.to_str().ok_or("Ruta PDF no representable")?;
    let normal = if let Some(unc) = path.strip_prefix(r"\\?\UNC\") {
        format!(r"\\{unc}")
    } else {
        path.strip_prefix(r"\\?\").unwrap_or(path).to_owned()
    };
    let render = (|| -> windows::core::Result<RenderedPdf> {
        let file = StorageFile::GetFileFromPathAsync(&HSTRING::from(normal))?.get()?;
        let document = PdfDocument::LoadFromFileAsync(&file)?.get()?;
        let page_count = document.PageCount()?;
        if page_count == 0 {
            return Err(windows::core::Error::new(
                windows::core::HRESULT(0x80004005u32 as i32),
                "PDF sin páginas",
            ));
        }
        let page_index = page_index.min(page_count - 1);
        let page = document.GetPage(page_index)?;
        let size = page.Size()?;
        let scale = ((width.clamp(320, 2048) as f32) / size.Width.max(1.0))
            .min(4096.0 / size.Height.max(1.0));
        let options = PdfPageRenderOptions::new()?;
        options.SetDestinationWidth((size.Width * scale).round().max(1.0) as u32)?;
        options.SetDestinationHeight((size.Height * scale).round().max(1.0) as u32)?;
        let stream = InMemoryRandomAccessStream::new()?;
        page.RenderWithOptionsToStreamAsync(&stream, &options)?
            .get()?;
        let length = stream.Size()?;
        if length > 64 * 1024 * 1024 {
            return Err(windows::core::Error::new(
                windows::core::HRESULT(0x80004005u32 as i32),
                "Página PDF demasiado grande",
            ));
        }
        let reader = DataReader::CreateDataReader(&stream.GetInputStreamAt(0)?)?;
        reader.LoadAsync(length as u32)?.get()?;
        let mut png = vec![0; length as usize];
        reader.ReadBytes(&mut png)?;
        page.Close()?;
        Ok(RenderedPdf {
            page_index,
            page_count,
            png,
        })
    })();
    render.map_err(|e| format!("No se pudo abrir/renderizar el PDF: {e}"))
}

#[cfg(not(windows))]
pub fn render_page(_path: &Path, _page_index: u32, _width: u32) -> Result<RenderedPdf, String> {
    Err("El lector PDF requiere Windows".into())
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;
    fn fixture() -> Vec<u8> {
        let streams = [
            "BT /F1 18 Tf 40 160 Td (MyCalibro page 1) Tj ET",
            "BT /F1 18 Tf 40 160 Td (MyCalibro page 2) Tj ET",
        ];
        let objects=vec![
            "<< /Type /Catalog /Pages 2 0 R >>".to_owned(),
            "<< /Type /Pages /Count 2 /Kids [3 0 R 4 0 R] >>".into(),
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 300 200] /Resources << /Font << /F1 5 0 R >> >> /Contents 6 0 R >>".into(),
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 300 200] /Resources << /Font << /F1 5 0 R >> >> /Contents 7 0 R >>".into(),
            "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>".into(),
            format!("<< /Length {} >>\nstream\n{}\nendstream",streams[0].len(),streams[0]),
            format!("<< /Length {} >>\nstream\n{}\nendstream",streams[1].len(),streams[1]),
        ];
        let mut pdf = b"%PDF-1.4\n".to_vec();
        let mut offsets = vec![0];
        for (index, object) in objects.iter().enumerate() {
            offsets.push(pdf.len());
            pdf.extend(format!("{} 0 obj\n{object}\nendobj\n", index + 1).as_bytes());
        }
        let xref = pdf.len();
        pdf.extend(format!("xref\n0 {}\n0000000000 65535 f \n", offsets.len()).as_bytes());
        for offset in &offsets[1..] {
            pdf.extend(format!("{offset:010} 00000 n \n").as_bytes());
        }
        pdf.extend(
            format!(
                "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n",
                offsets.len()
            )
            .as_bytes(),
        );
        pdf
    }
    #[test]
    fn native_windows_pdf_renders_real_pages_without_modifying_source() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("fixture.pdf");
        let source = fixture();
        std::fs::write(&path, &source).unwrap();
        let first = render_page(&path, 0, 640).unwrap();
        assert_eq!(first.page_count, 2);
        assert_eq!(first.page_index, 0);
        let image = image::load_from_memory(&first.png).unwrap();
        assert_eq!(image.width(), 640);
        assert!(image.height() > 0);
        let last = render_page(&path, 999, 640).unwrap();
        assert_eq!(last.page_index, 1);
        assert_ne!(first.png, last.png);
        assert_eq!(std::fs::read(&path).unwrap(), source);
        std::fs::write(&path, b"not a PDF").unwrap();
        assert!(render_page(&path, 0, 640).is_err());
    }
}
