use super::codec::{decode_row, decode_table, encode_row, encode_table};
use super::error::DbError;
use super::page::{PageId, PageKind, RecordId, SlottedPage};
use super::pager::Pager;
use super::row::Row;
use super::schema::Schema;
use super::table::Table;
use super::value::Value;
use std::cell::{Ref, RefCell};
use std::collections::HashMap;
use std::fmt::format;
use std::hash::Hash;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static TEMP_DATABASE_NUMBER: AtomicU64 = AtomicU64::new(0);


pub struct Database {
    tables: HashMap<String, Table>, 
    pager: RefCell<Pager>,
    temporary_path: Option<PathBuf>
}

impl std::fmt::Debug for Database {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Database")
            .field("tables", &self.tables)
            .field("temporary_path", &self.temporary_path)
            .finish_non_exhaustive()
    }
}

impl Default for Database {
    fn default() -> Self {
        Self::new()
    }
}
impl Database{
    /// Creates a throw-away on-disk database for examples and older callers.
    /// Use `Database::create` when the file should survive this value.
    pub fn new() -> Self {
        let number = TEMP_DATABASE_NUMBER.fetch_add(1, Ordering::Relaxed);
        let path= std::env::temp_dir().join(format!("lidb-{}--{number}.db", std::process::id()));
        let pager = Pager::create(&path).expect("Could not create temporary LiDB");
        Self {
            tables: HashMap::new(), 
            pager: RefCell::new(pager),
            temporary_path: Some(path),
        }
    }

    pub fn create( path: impl AsRef<Path>) -> Result< Self, DbError> {
        // Creates a durable database file, with refusing to overwrite an existing one
        let pager = Pager::create(path.as_ref())?;
        Ok(Self { tables: HashMap::new(), pager: RefCell::new(pager), temporary_path: None, })
    }

    //Openms a existing file and rebuilds the in memory schema catalog from page 1. Rows remain on disk until scanned
    pub fn open(path: impl AsRef<Path>) -> Result<Self, DbError> {
        let mut pager = Pager::open(path.as_ref())?;
        let catalog = pager.read_page(PageId(1), PageKind::Catalog)?;
        let mut tables = HashMap::new(); 
        for (_, bytes) in catalog.records()? {
            let table = decode_table(bytes)?; 
            let name = table.name().to_string();
            if tables.insert(name.clone(), table).is_some() { 
                return Err(DbError::Corrupt(format!("Duplicate catalog table '{name}'")));
            } 
        }

        Ok(Self { tables, pager: RefCell::new(pager), temporary_path: None })

    }
    pub fn flush(&self) -> Result<(), DbError> { 
        self.pager.borrow_mut().flush()
    }
    /// Explicitly synchronizes the file. Dropping a database also flushes, but
    /// `close` lets applications observe a possible I/O error.
    pub fn close(self) -> Result<(), DbError> { 
        self.flush()
    }

    pub fn create_table(
        &mut self,
        name: impl Into<String>,
        schema: Schema, 
    ) -> Result<(), DbError>
    {
        let name = name.into();

        if self.tables.contains_key(&name){
            return Err(DbError::TableAlreadyExists { table: name });
        }

        // Both the map key and Table own the name; keep the local value for
        // rolling the insertion back if persisting the catalog fails.
        self.tables
            .insert(name.clone(), Table::new(name.clone(), schema));
        if let Err(error) = self.write_catalog() { 
            self.tables.remove(&name);
            return Err(error);
        }
        Ok(())
    }

    pub fn table_count(&self) -> usize {
        self.tables.len()
    }

    pub fn drop_table(&mut self, name: &str) -> Result<Table, DbError> {
        let table = self.tables.remove(name)
            .ok_or_else(|| DbError::TableNotFound { table: name.to_string() })?;
        if let Err(error) = self.write_catalog() {
            self.tables.insert(name.to_string(), table);
            return Err(error);
        }

        self.flush()?; 
        Ok(table)

    }

    pub fn table(&self, name: &str) -> Result<&Table, DbError>{
        self.tables.get(name)
            .ok_or_else(|| DbError::TableNotFound { table: name.to_string() })
    }

    pub fn table_mut(&mut self, name: &str) -> Result<&mut Table, DbError>{
        self.tables
            .get_mut(name)
            .ok_or_else(|| DbError::TableNotFound { table: name.to_string() })
    }

    pub fn insert(&mut self, table_name: &str, values: Vec<Value>) -> Result<RecordId, DbError> {
        self.insert_row(table_name, Row::new(values))
    }

    pub(crate) fn insert_row(&mut self, table_name: &str, row: Row) -> Result<RecordId, DbError> {
        let table = self.table(table_name)?;
        table.schema().validate_row(table.name(), &row)?;
        let encoded = encode_row(&row)?;
        let first_page = table.first_page();

        let (record_id, new_first) = {
            let mut pager = self.pager.borrow_mut();
            append_record(&mut pager, first_page, &encoded)?
        };
        let table = self.table_mut(table_name)?;
        table.set_storage(new_first, table.row_count() as u64 + 1);
        self.write_catalog()?;
        self.flush()?;
        Ok(record_id)
    }



    /// Returns owned rows because page buffers cannot safely lend references
    /// after the pager moves on to another page.
    ///
    /// TODO(storage): This intentionally materializes the full table for the
    /// small teaching implementation. Add a page-backed scan cursor for SQL
    /// execution so large scans decode one page at a time instead.
    pub fn select_all(&self, table_name: &str) -> Result<Vec<Row>, DbError> {
        let table = self.table(table_name)?;
        let mut rows = Vec::with_capacity(table.row_count());
        let mut next = table.first_page();
        let mut pager = self.pager.borrow_mut();
        let mut visited = std::collections::HashSet::new();
        while let Some(page_id) = next {
            if !visited.insert(page_id) {
                return Err(DbError::Corrupt(format!(
                    "cycle in page chain at page {}",
                    page_id.0
                )));
            }
            let page = pager.read_page(page_id, PageKind::Data)?;
            for (_, bytes) in page.records()? {
                let row = decode_row(bytes)?;
                table
                    .schema()
                    .validate_row(table.name(), &row)
                    .map_err(|error| {
                        DbError::Corrupt(format!(
                            "row on page {} violates its schema: {error}",
                            page_id.0
                        ))
                    })?;
                rows.push(row);
            }
            next = page.next_page();
        }
        if rows.len() != table.row_count() {
            return Err(DbError::Corrupt(format!(
                "table '{}' catalog says {} rows, found {}",
                table.name(),
                table.row_count(),
                rows.len()
            )));
        }
        Ok(rows)
    }

    pub fn select_where<F>(&self, table_name: &str, predicate: F) -> Result<Vec<Row>, DbError>
    where
        F: Fn(&Row) -> bool,
    {
        Ok(self
            .select_all(table_name)?
            .into_iter()
            .filter(predicate)
            .collect())
    }

    pub fn update_where<F>(
        &mut self,
        table_name: &str,
        predicate: F,
        column_name: &str,
        new_value: Value,
    ) -> Result<usize, DbError>
    where
        F: Fn(&Row) -> bool,
    {
        let table = self.table(table_name)?;
        let index =
            table
                .schema()
                .column_index(column_name)
                .ok_or_else(|| DbError::ColumnNotFound {
                    table: table_name.to_string(),
                    column: column_name.to_string(),
                })?;
        table
            .schema()
            .validate_value(table_name, &table.schema().columns()[index], &new_value)?;
        let mut rows = self.select_all(table_name)?;
        let mut count = 0;
        for row in &mut rows {
            if predicate(row) {
                row[index] = new_value.clone();
                count += 1;
            }
        }
        if count != 0 {
            self.replace_rows(table_name, rows)?;
        }
        Ok(count)
    }

    pub fn delete_where<F>(&mut self, table_name: &str, predicate: F) -> Result<usize, DbError>
    where
        F: Fn(&Row) -> bool,
    {
        let mut rows = self.select_all(table_name)?;
        let old_len = rows.len();
        rows.retain(|row| !predicate(row));
        let deleted = old_len - rows.len();
        if deleted != 0 {
            self.replace_rows(table_name, rows)?;
        }
        Ok(deleted)
    }

    /// Writes a complete replacement chain before changing the catalog. Old
    /// pages become unreachable; a future free-list phase can recycle them.
    ///
    /// TODO(storage): Replace this copy-on-rewrite approach with RecordId-based
    /// in-place updates/deletes, plus a free list for reclaiming empty pages.
    pub(crate) fn replace_rows(&mut self, table_name: &str, rows: Vec<Row>) -> Result<(), DbError> {
        let table = self.table(table_name)?;
        for row in &rows {
            table.schema().validate_row(table.name(), row)?;
        }
        let (first, count) = {
            let mut pager = self.pager.borrow_mut();
            write_row_chain(&mut pager, &rows)?
        };
        self.table_mut(table_name)?.set_storage(first, count);
        self.write_catalog()?;
        self.flush()
    }

    pub fn execute(&mut self, sql: &str) -> Result<Vec<Row>, crate::DatabaseError> {
        crate::executor::execute_sql(self, sql)
    }

    pub fn execute_batch(&mut self, sql: &str) -> Result<Vec<Vec<Row>>, crate::DatabaseError> {
        crate::executor::execute_sql_batch(self, sql)
    }

    fn write_catalog(&self) -> Result<(), DbError> {
        let mut catalog = SlottedPage::new(PageKind::Catalog); 
        let mut names = self.tables.keys().collect::<Vec<_>>(); 
        names.sort_unstable();
        for name in names{
            let encoded = encode_table(&self.tables[name])?;
            catalog.insert(&encoded).map_err(|error| match error {
                DbError::PageFull | DbError::RecordTooLarge { .. } => DbError::CatalogFull,
                other => other,
            })?;
        }

        self.pager.borrow_mut().write_page(PageId(1), &catalog)
    }

}

impl Drop for Database {
    fn drop (&mut self) {
        let _ = self.pager.get_mut().flush(); 
        if let Some( path) = &self.temporary_path { 
            let _= std::fs::remove_file(path); 
        }
    }
}


fn append_record(
    pager: &mut Pager,
    first: Option<PageId>,
    bytes: &[u8],
) -> Result<(RecordId, Option<PageId>), DbError> {
    let Some(mut page_id) = first else {
        let (id, mut page) = pager.allocate(PageKind::Data)?;
        let slot_id = page.insert(bytes)?;
        pager.write_page(id, &page)?;
        return Ok((
            RecordId {
                page_id: id,
                slot_id,
            },
            Some(id),
        ));
    };
    loop {
        let mut page = pager.read_page(page_id, PageKind::Data)?;
        match page.insert(bytes) {
            Ok(slot_id) => {
                pager.write_page(page_id, &page)?;
                return Ok((RecordId { page_id, slot_id }, first));
            }
            Err(DbError::PageFull) => match page.next_page() {
                Some(next) => page_id = next,
                None => {
                    let (new_id, mut new_page) = pager.allocate(PageKind::Data)?;
                    let slot_id = new_page.insert(bytes)?;
                    pager.write_page(new_id, &new_page)?;
                    page.set_next_page(Some(new_id));
                    pager.write_page(page_id, &page)?;
                    return Ok((
                        RecordId {
                            page_id: new_id,
                            slot_id,
                        },
                        first,
                    ));
                }
            },
            Err(error) => return Err(error),
        }
    }
}

fn write_row_chain(pager: &mut Pager, rows: &[Row]) -> Result<(Option<PageId>, u64), DbError> {
    let mut first = None;
    for row in rows {
        let bytes = encode_row(row)?;
        let (_, updated_first) = append_record(pager, first, &bytes)?;
        first = updated_first;
    }
    Ok((first, rows.len() as u64))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Column, DataType};

    fn user_schema() -> Schema {
        Schema::new(vec![
            Column::required("id", DataType::Integer),
            Column::required("name", DataType::Text),
        ])
        .unwrap()
    }

    #[test]
    fn durable_database_reopens_rows_and_catalog() {
        let path = std::env::temp_dir().join(format!(
            "tinyrel-reopen-{}-{}.db",
            std::process::id(),
            TEMP_DATABASE_NUMBER.fetch_add(1, Ordering::Relaxed)
        ));
        {
            let mut db = Database::create(&path).unwrap();
            db.create_table("users", user_schema()).unwrap();
            db.insert("users", vec![Value::Integer(1), Value::text("Liam")])
                .unwrap();
        }
        let db = Database::open(&path).unwrap();
        assert_eq!(
            db.select_all("users").unwrap(),
            vec![Row::new(vec![Value::Integer(1), Value::text("Liam")])]
        );
        drop(db);
        std::fs::remove_file(path).unwrap();
    }
}
