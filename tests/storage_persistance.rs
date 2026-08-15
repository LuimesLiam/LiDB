use std::sync::atomic::{AtomicU64, Ordering};
use LiDB::{Database, DbError, PAGE_SIZE, Row, Value};

static FILE_NUMBER: AtomicU64 = AtomicU64::new(0);

fn database_path(test: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!(
        "tinyrel-{test}-{}-{}.db",
        std::process::id(),
        FILE_NUMBER.fetch_add(1, Ordering::Relaxed)
    ))
}

#[test]
fn create_close_reopen_preserves_catalog_and_rows_across_pages() {
    let path = database_path("persistence");
    {
        let mut database = Database::create(&path).unwrap();
        database
            .execute("CREATE TABLE notes (id INTEGER, body TEXT);")
            .unwrap();
        // These payloads cannot all share one 4 KiB page.
        for id in 0..12 {
            database
                .insert(
                    "notes",
                    vec![Value::Integer(id), Value::text("x".repeat(700))],
                )
                .unwrap();
        }
        database.close().unwrap();
    }

    assert!(std::fs::metadata(&path).unwrap().len() > (3 * PAGE_SIZE) as u64);
    {
        let mut reopened = Database::open(&path).unwrap();
        assert_eq!(reopened.table("notes").unwrap().row_count(), 12);
        assert_eq!(
            reopened.select_all("notes").unwrap()[11][0],
            Value::Integer(11)
        );

        reopened
            .execute("UPDATE notes SET body = 'persisted' WHERE id = 5;")
            .unwrap();
        reopened.execute("DELETE FROM notes WHERE id < 2;").unwrap();
    }

    let reopened = Database::open(&path).unwrap();
    let rows = reopened.select_all("notes").unwrap();
    assert_eq!(rows.len(), 10);
    assert_eq!(
        rows[3],
        Row::new(vec![Value::Integer(5), Value::text("persisted")])
    );
    drop(reopened);
    std::fs::remove_file(path).unwrap();
}

#[test]
fn one_row_must_fit_on_one_page() {
    let path = database_path("oversize");
    let mut database = Database::create(&path).unwrap();
    database.execute("CREATE TABLE texts (body TEXT);").unwrap();
    let result = database.insert("texts", vec![Value::text("x".repeat(PAGE_SIZE))]);
    assert!(matches!(result, Err(DbError::RecordTooLarge { .. })));
    drop(database);
    std::fs::remove_file(path).unwrap();
}
