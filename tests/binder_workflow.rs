use LiDB::{
    BinaryOperator, BindError, Binder, BoundExpression, BoundStatement, Column, DataType, Database,
    Schema, parse_sql,
};

fn database_with_users() -> Database {
    let mut database = Database::new();
    database
        .create_table(
            "users",
            Schema::new(vec![
                Column::required("id", DataType::Integer),
                Column::required("name", DataType::Text),
                Column::nullable("active", DataType::Boolean),
            ])
            .unwrap(),
        )
        .unwrap();
    database
}

#[test]
fn resolves_columns_to_typed_row_indexes() {
    let database = database_with_users();
    let statement = parse_sql("SELECT name FROM users WHERE id = 1;").unwrap();
    let bound = Binder::new(&database).bind(statement).unwrap();

    let BoundStatement::Select(select) = bound else {
        panic!("expected a bound SELECT");
    };
    assert_eq!(
        select.projections,
        vec![BoundExpression::Column {
            column_index: 1,
            data_type: DataType::Text,
        }]
    );
    assert!(matches!(
        select.filter,
        Some(BoundExpression::Binary {
            operator: BinaryOperator::Equal,
            data_type: DataType::Boolean,
            ..
        })
    ));
}

#[test]
fn rejects_a_nonexistent_column() {
    let database = database_with_users();
    let statement = parse_sql("SELECT nonexistent FROM users;").unwrap();

    assert_eq!(
        Binder::new(&database).bind(statement),
        Err(BindError::ColumnNotFound {
            table: "users".to_string(),
            column: "nonexistent".to_string(),
        })
    );
}

#[test]
fn rejects_an_insert_with_the_wrong_type() {
    let database = database_with_users();
    let statement = parse_sql("INSERT INTO users VALUES ('wrong type', 'Liam', true);").unwrap();

    assert!(matches!(
        Binder::new(&database).bind(statement),
        Err(BindError::TypeMismatch {
            expected: DataType::Integer,
            actual: DataType::Text,
            ..
        })
    ));
}

#[test]
fn rejects_incompatible_comparison_types() {
    let database = database_with_users();
    let statement = parse_sql("SELECT name FROM users WHERE id = 'hello';").unwrap();
    let error = Binder::new(&database).bind(statement).unwrap_err();

    assert_eq!(
        error,
        BindError::IncompatibleTypes {
            operator: BinaryOperator::Equal,
            left: DataType::Integer,
            right: DataType::Text,
        }
    );
    assert_eq!(error.to_string(), "Cannot compare INTEGER with TEXT");
}

#[test]
fn expands_wildcards_and_requires_boolean_filters() {
    let database = database_with_users();
    let wildcard = parse_sql("SELECT * FROM users;").unwrap();
    let BoundStatement::Select(select) = Binder::new(&database).bind(wildcard).unwrap() else {
        panic!("expected a bound SELECT");
    };
    assert_eq!(select.projections.len(), 3);

    let invalid_filter = parse_sql("DELETE FROM users WHERE id + 1;").unwrap();
    assert!(matches!(
        Binder::new(&database).bind(invalid_filter),
        Err(BindError::ExpectedBoolean {
            clause: "WHERE expression",
            actual: Some(DataType::Integer),
        })
    ));
}
