//! One-time import metadata from an isolated, verified snapshot of Calibre's database.
//! Never opens the source database with SQLite and never modifies the source library.
use crate::storage;
use rusqlite::{Connection, OpenFlags};
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

#[derive(Clone, Debug)]
pub struct CalibreRecord {
    pub calibre_id: i64,
    pub title: String,
    pub author: String,
    pub year: String,
    pub description: String,
    pub tags: Vec<String>,
    pub identifiers: BTreeMap<String, String>,
    pub formats: Vec<PathBuf>,
    pub cover: Option<PathBuf>,
}

#[derive(Debug, Default)]
pub struct CalibreScan {
    pub records: Vec<CalibreRecord>,
    pub warnings: Vec<String>,
}

fn no_active_journal(root: &Path) -> Result<(), String> {
    for name in ["metadata.db-wal", "metadata.db-journal"] {
        match fs::metadata(root.join(name)) {
            Ok(meta) if meta.len()>0 => return Err("Cierre Calibre y deje que finalice sus transacciones antes de importar esta biblioteca".into()),
            Ok(_) => (),
            Err(e) if e.kind()==std::io::ErrorKind::NotFound => (),
            Err(e) => return Err(e.to_string()),
        }
    }
    Ok(())
}

fn strings(connection: &Connection, sql: &str, id: i64) -> Result<Vec<String>, String> {
    let mut query = connection
        .prepare(sql)
        .map_err(|e| format!("Esquema Calibre no compatible: {e}"))?;
    let values = query
        .query_map([id], |row| row.get::<_, String>(0))
        .map_err(|e| e.to_string())?;
    values
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())
}

/// Resolve a format within the selected Calibre library without leaving its root.
/// Calibre can retain a file name with different letter case after a library copy
/// from a case-insensitive filesystem. Never guess between ambiguous matches.
fn resolve_format(root: &Path, relative: &Path) -> Result<Option<PathBuf>, String> {
    let exact = storage::safe_join(root, relative)?;
    if exact.is_file() {
        return Ok(Some(exact));
    }

    let mut resolved = PathBuf::new();
    for component in relative.components() {
        let std::path::Component::Normal(name) = component else {
            return Err("Ruta de formato Calibre no válida".into());
        };
        let directory = if resolved.as_os_str().is_empty() {
            root.to_path_buf()
        } else {
            storage::safe_join(root, &resolved)?
        };
        let entries = match fs::read_dir(&directory) {
            Ok(entries) => entries,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(error.to_string()),
        };
        let mut matches = Vec::new();
        for entry in entries {
            let entry = entry.map_err(|error| error.to_string())?;
            if entry
                .file_name()
                .to_string_lossy()
                .eq_ignore_ascii_case(&name.to_string_lossy())
            {
                matches.push(entry.file_name());
            }
        }
        match matches.len() {
            0 => return Ok(None),
            1 => resolved.push(&matches[0]),
            _ => return Err("Nombre ambiguo: varios archivos coinciden ignorando mayúsculas".into()),
        }
    }
    let candidate = storage::safe_join(root, &resolved)?;
    Ok(candidate.is_file().then_some(candidate))
}

pub fn read_library(root: &Path) -> Result<CalibreScan, String> {
    let root = fs::canonicalize(root).map_err(|e| e.to_string())?;
    no_active_journal(&root)?;
    let source = storage::safe_join(&root, Path::new("metadata.db"))?;
    let stage = tempfile::tempdir().map_err(|e| e.to_string())?;
    let snapshot = stage.path().join("metadata.db");
    storage::copy_new(&source, &snapshot)?;
    no_active_journal(&root)?;
    let database = Connection::open_with_flags(snapshot, OpenFlags::SQLITE_OPEN_READ_ONLY)
        .map_err(|e| e.to_string())?;
    let integrity: String = database
        .query_row("PRAGMA quick_check", [], |row| row.get(0))
        .map_err(|e| e.to_string())?;
    if integrity != "ok" {
        return Err(format!(
            "La copia de metadata.db no supera integridad: {integrity}"
        ));
    }
    let mut query = database
        .prepare("SELECT id,title,path,COALESCE(pubdate,'') FROM books ORDER BY id")
        .map_err(|e| format!("Esquema Calibre no compatible: {e}"))?;
    let rows = query
        .query_map([], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
            ))
        })
        .map_err(|e| e.to_string())?;
    let mut result = CalibreScan::default();
    let mut declared_formats = 0usize;
    let mut accessible_formats = 0usize;
    let mut sample_expected = None;
    for row in rows {
        let (id, title, relative, published) = row.map_err(|e| e.to_string())?;
        let authors=strings(&database,"SELECT a.name FROM authors a JOIN books_authors_link l ON a.id=l.author WHERE l.book=? ORDER BY l.id",id)?;
        let tags=strings(&database,"SELECT t.name FROM tags t JOIN books_tags_link l ON t.id=l.tag WHERE l.book=? ORDER BY t.name",id)?;
        let comments = strings(&database, "SELECT text FROM comments WHERE book=?", id)?;
        let mut record = CalibreRecord {
            calibre_id: id,
            title,
            author: authors.join(" & "),
            year: published
                .get(..4)
                .filter(|s| s.chars().all(|c| c.is_ascii_digit()) && *s != "0101")
                .unwrap_or("")
                .into(),
            description: comments.first().cloned().unwrap_or_default(),
            tags,
            identifiers: BTreeMap::new(),
            formats: vec![],
            cover: None,
        };
        let mut identifiers = database
            .prepare("SELECT type,val FROM identifiers WHERE book=?")
            .map_err(|e| e.to_string())?;
        for identifier in identifiers
            .query_map([id], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })
            .map_err(|e| e.to_string())?
        {
            let (kind, value) = identifier.map_err(|e| e.to_string())?;
            record.identifiers.insert(kind, value);
        }
        let mut formats = database
            .prepare("SELECT name,format FROM data WHERE book=? ORDER BY format")
            .map_err(|e| e.to_string())?;
        for format in formats
            .query_map([id], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })
            .map_err(|e| e.to_string())?
        {
            let (name, extension) = format.map_err(|e| e.to_string())?;
            declared_formats += 1;
            if !extension.chars().all(|c| c.is_ascii_alphanumeric()) || extension.is_empty() {
                result
                    .warnings
                    .push(format!("Libro {id}: extensión no válida"));
                continue;
            }
            let relative =
                PathBuf::from(&relative).join(format!("{name}.{}", extension.to_ascii_lowercase()));
            if sample_expected.is_none() {
                sample_expected = Some(root.join(&relative).display().to_string());
            }
            match resolve_format(&root, &relative) {
                Ok(Some(path)) => {
                    if path != root.join(&relative) {
                        result.warnings.push(format!(
                            "Libro {id}: archivo localizado con diferencias de mayúsculas: {}",
                            path.display()
                        ));
                    }
                    record.formats.push(path);
                    accessible_formats += 1;
                }
                Ok(None) => result.warnings.push(format!(
                    "Libro {id}: no existe el archivo esperado {}",
                    root.join(&relative).display()
                )),
                Err(e) => result.warnings.push(format!("Libro {id}: {e}")),
            }
        }
        if let Ok(cover) = storage::safe_join(&root, &PathBuf::from(&relative).join("cover.jpg")) {
            if cover.is_file() {
                record.cover = Some(cover);
            }
        }
        if record.formats.is_empty() {
            result.warnings.push(format!(
                "Libro {id} sin formatos accesibles: {}",
                record.title
            ));
        }
        result.records.push(record);
    }
    if !result.records.is_empty() && accessible_formats == 0 {
        if declared_formats == 0 {
            return Err(format!(
                "La biblioteca seleccionada contiene {} registros Calibre, pero metadata.db no registra ningún archivo en la tabla data. Compruebe que es la biblioteca correcta y que sus libros están disponibles; no se ha importado nada.",
                result.records.len()
            ));
        }
        return Err(format!(
            "Calibre registra {} libros y {declared_formats} formatos, pero ninguno es accesible desde la carpeta seleccionada {}. Ejemplo de ruta esperada: {}. Compruebe que la carpeta contiene también las subcarpetas de autores y los archivos de los libros, que están descargados y que no ha seleccionado una copia aislada de metadata.db. No se ha modificado Calibre ni importado una biblioteca vacía.",
            result.records.len(),
            root.display(),
            sample_expected.unwrap_or_default()
        ));
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn library() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        let database = Connection::open(dir.path().join("metadata.db")).unwrap();
        database.execute_batch("CREATE TABLE books(id INTEGER,title TEXT,path TEXT,pubdate TEXT);
          CREATE TABLE authors(id INTEGER,name TEXT); CREATE TABLE books_authors_link(id INTEGER,book INTEGER,author INTEGER);
          CREATE TABLE tags(id INTEGER,name TEXT); CREATE TABLE books_tags_link(book INTEGER,tag INTEGER);
          CREATE TABLE comments(book INTEGER,text TEXT); CREATE TABLE identifiers(book INTEGER,type TEXT,val TEXT);
          CREATE TABLE data(book INTEGER,name TEXT,format TEXT);
          INSERT INTO books VALUES(1,'Título original','Autor/Titulo (1)','2020-01-01');
          INSERT INTO authors VALUES(1,'Autor original'); INSERT INTO books_authors_link VALUES(1,1,1);
          INSERT INTO tags VALUES(1,'Historia'); INSERT INTO books_tags_link VALUES(1,1);
          INSERT INTO comments VALUES(1,'Descripción original'); INSERT INTO identifiers VALUES(1,'isbn','1234');
          INSERT INTO data VALUES(1,'Titulo','EPUB'); INSERT INTO data VALUES(1,'Titulo','PDF');").unwrap();
        drop(database);
        fs::create_dir_all(dir.path().join("Autor/Titulo (1)")).unwrap();
        fs::write(
            dir.path().join("Autor/Titulo (1)/Titulo.epub"),
            b"epub fixture",
        )
        .unwrap();
        fs::write(
            dir.path().join("Autor/Titulo (1)/cover.jpg"),
            b"cover fixture",
        )
        .unwrap();
        dir
    }
    #[test]
    fn reads_metadata_and_reports_missing_formats_without_changing_origin() {
        let dir = library();
        let before = storage::hash_file(&dir.path().join("metadata.db")).unwrap();
        let scan = read_library(dir.path()).unwrap();
        assert_eq!(scan.records.len(), 1);
        let book = &scan.records[0];
        assert_eq!(book.calibre_id, 1);
        assert_eq!(book.title, "Título original");
        assert_eq!(book.author, "Autor original");
        assert_eq!(book.year, "2020");
        assert_eq!(book.tags, vec!["Historia"]);
        assert_eq!(book.identifiers["isbn"], "1234");
        assert_eq!(book.description, "Descripción original");
        assert!(book.cover.as_ref().unwrap().is_file());
        assert_eq!(book.formats.len(), 1);
        assert_eq!(scan.warnings.len(), 1);
        assert_eq!(
            storage::hash_file(&dir.path().join("metadata.db")).unwrap(),
            before
        );
        assert!(!dir.path().join("metadata.db-journal").exists());
        assert!(!dir.path().join("metadata.db-shm").exists());
    }
    #[test]
    fn rejects_live_transactions_and_invalid_database_without_writing() {
        let dir = library();
        fs::write(dir.path().join("metadata.db-wal"), b"active").unwrap();
        assert!(read_library(dir.path()).is_err());
        fs::remove_file(dir.path().join("metadata.db-wal")).unwrap();
        fs::write(dir.path().join("metadata.db"), b"corrupt").unwrap();
        assert!(read_library(dir.path()).is_err());
        assert_eq!(
            fs::read(dir.path().join("metadata.db")).unwrap(),
            b"corrupt"
        );
    }
    #[test]
    fn database_paths_cannot_escape_source_library() {
        let dir = library();
        let database = Connection::open(dir.path().join("metadata.db")).unwrap();
        database
            .execute("UPDATE books SET path='../outside'", [])
            .unwrap();
        drop(database);
        let error = read_library(dir.path()).unwrap_err();
        assert!(error.contains("ninguno es accesible"));
    }

    #[test]
    fn resolves_differently_cased_names_without_changing_calibre() {
        let dir = library();
        let database = Connection::open(dir.path().join("metadata.db")).unwrap();
        database
            .execute("UPDATE data SET name='titulo' WHERE format='EPUB'", [])
            .unwrap();
        drop(database);
        let before = storage::hash_file(&dir.path().join("metadata.db")).unwrap();
        let scan = read_library(dir.path()).unwrap();
        assert_eq!(scan.records[0].formats.len(), 1);
        assert_eq!(
            scan.records[0].formats[0],
            dir.path().join("Autor/Titulo (1)/Titulo.epub")
        );
        assert_eq!(
            storage::hash_file(&dir.path().join("metadata.db")).unwrap(),
            before
        );
    }

    #[test]
    fn reports_missing_ebook_files_with_actionable_path() {
        let dir = library();
        fs::remove_file(dir.path().join("Autor/Titulo (1)/Titulo.epub")).unwrap();
        let error = read_library(dir.path()).unwrap_err();
        assert!(error.contains("ninguno es accesible"));
        assert!(error.contains("Autor"));
        assert!(error.contains("Titulo.epub"));
        assert!(error.contains("No se ha modificado Calibre"));
    }

    #[test]
    fn reports_database_without_registered_formats() {
        let dir = library();
        let database = Connection::open(dir.path().join("metadata.db")).unwrap();
        database.execute("DELETE FROM data", []).unwrap();
        drop(database);
        let error = read_library(dir.path()).unwrap_err();
        assert!(error.contains("no registra ningún archivo"));
    }
}
