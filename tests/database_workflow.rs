use LiDB::{Column, DataType, Database, DbError, Schema, Value};

fn users_schema() -> Schema {
    Schema::new(vec![
        Column::required("id", DataType::Integer),
        Column::required("name", DataType::Text),
        Column::nullable("email", DataType::Text),
        Column::required("active", DataType::Boolean),
    ])
    .unwrap()
}

#[test]
fn complete_crud_workflow() {
    let mut db = Database::new();
    db.create_table("users", users_schema()).unwrap();

    db.insert(
        "users",
        vec![
            Value::Integer(1),
            Value::text("Liam"),
            Value::Null,
            Value::Boolean(true),
        ],
    )
    .unwrap();

    db.insert(
        "users",
        vec![
            Value::Integer(2),
            Value::text("Alice"),
            Value::text("alice@example.com"),
            Value::Boolean(false),
        ],
    )
    .unwrap();

    let all_users = db.select_all("users").unwrap();
    assert_eq!(all_users.len(), 2);

    let active_users = db
        .select_where("users", |row| row[3] == Value::Boolean(true))
        .unwrap();
    assert_eq!(active_users.len(), 1);
    assert_eq!(active_users[0][1], Value::text("Liam"));

    let updated = db
        .update_where(
            "users",
            |row| row[0] == Value::Integer(2),
            "active",
            Value::Boolean(true),
        )
        .unwrap();
    assert_eq!(updated, 1);

    let now_active = db
        .select_where("users", |row| row[3] == Value::Boolean(true))
        .unwrap();
    assert_eq!(now_active.len(), 2);

    let deleted = db
        .delete_where("users", |row| row[0] == Value::Integer(1))
        .unwrap();
    assert_eq!(deleted, 1);

    let remaining = db.select_all("users").unwrap();
    assert_eq!(remaining.len(), 1);
    assert_eq!(remaining[0][1], Value::text("Alice"));
}

#[test]
fn invalid_insert_does_not_change_the_table() {
    let mut db = Database::new();
    db.create_table("users", users_schema()).unwrap();

    let result = db.insert(
        "users",
        vec![
            Value::text("wrong id type"),
            Value::text("Liam"),
            Value::Null,
            Value::Boolean(true),
        ],
    );

    assert!(matches!(result, Err(DbError::TypeMismatch { .. })));
    assert_eq!(db.select_all("users").unwrap().len(), 0);
}

#[test]
fn null_is_rejected_for_required_columns() {
    let mut db = Database::new();
    db.create_table("users", users_schema()).unwrap();

    let result = db.insert(
        "users",
        vec![
            Value::Integer(1),
            Value::Null,
            Value::Null,
            Value::Boolean(true),
        ],
    );

    assert_eq!(
        result,
        Err(DbError::NullNotAllowed {
            table: "users".to_string(),
            column: "name".to_string(),
        })
    );
}
