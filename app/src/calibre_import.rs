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
            if !extension.chars().all(|c| c.is_ascii_alphanumeric()) || extension.is_empty() {
                result
                    .warnings
                    .push(format!("Libro {id}: extensión no válida"));
                continue;
            }
            let relative =
                PathBuf::from(&relative).join(format!("{name}.{}", extension.to_ascii_lowercase()));
            match storage::safe_join(&root, &relative) {
                Ok(path) if path.is_file() => record.formats.push(path),
                Ok(path) => result
                    .warnings
                    .push(format!("Libro {id}: falta {}", path.display())),
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
        let scan = read_library(dir.path()).unwrap();
        assert!(scan.records[0].formats.is_empty());
        assert!(!scan.warnings.is_empty());
    }
}
