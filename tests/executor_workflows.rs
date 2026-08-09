use LiDB::{Database, Row, Value};

#[test]
fn sql_executes_a_complete_crud_workflow() {
    let mut database = Database::new();

    assert!(
        database
            .execute("CREATE TABLE users (id INTEGER, name TEXT, active BOOLEAN);")
            .unwrap()
            .is_empty()
    );
    database
        .execute("INSERT INTO users VALUES (1, 'Liam', true);")
        .unwrap();
    database
        .execute("INSERT INTO users VALUES (11, 'Alice', false);")
        .unwrap();
    database
        .execute("INSERT INTO users VALUES (12, 'Bob', true);")
        .unwrap();

    let rows = database
        .execute("SELECT name FROM users WHERE id > 10 AND name != 'Bob';")
        .unwrap();
    assert_eq!(rows, vec![Row::new(vec![Value::text("Alice")])]);

    let updated = database
        .execute("UPDATE users SET active = true, id = id + 1 WHERE name = 'Alice';")
        .unwrap();
    assert_eq!(updated.len(), 1);
    assert_eq!(updated[0][0], Value::Integer(12));
    assert_eq!(updated[0][2], Value::Boolean(true));

    let deleted = database
        .execute("DELETE FROM users WHERE id <= 1;")
        .unwrap();
    assert_eq!(deleted.len(), 1);
    assert_eq!(database.execute("SELECT * FROM users;").unwrap().len(), 2);
}

#[test]
fn projections_evaluate_expressions_and_null_filters_are_not_true() {
    let mut database = Database::new();
    database
        .execute_batch(
            "CREATE TABLE metrics (id INTEGER, flag BOOLEAN);\
             INSERT INTO metrics VALUES (1, true);",
        )
        .unwrap();

    assert_eq!(
        database
            .execute("SELECT id * 10 + 2 FROM metrics WHERE NULL = id;")
            .unwrap(),
        Vec::<Row>::new()
    );
    assert_eq!(
        database
            .execute("SELECT id * 10 + 2 FROM metrics WHERE flag;")
            .unwrap(),
        vec![Row::new(vec![Value::Integer(12)])]
    );
}

#[test]
fn a_failing_update_does_not_partially_modify_the_table() {
    let mut database = Database::new();
    database
        .execute_batch(
            "CREATE TABLE numbers (value INTEGER);\
             INSERT INTO numbers VALUES (1);\
             INSERT INTO numbers VALUES (0);",
        )
        .unwrap();

    assert!(
        database
            .execute("UPDATE numbers SET value = 10 / value;")
            .is_err()
    );
    assert_eq!(
        database.execute("SELECT value FROM numbers;").unwrap(),
        vec![
            Row::new(vec![Value::Integer(1)]),
            Row::new(vec![Value::Integer(0)]),
        ]
    );
}
