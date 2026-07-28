use LiDB::{Column, DataType, Database, Schema, Value, Lexer, Parser};

fn main() -> Result<(), Box<dyn std::error::Error>> {

    let sql = "SELECT id, name FROM users WHERE id >= 1 AND name != 'Bob';";
    println!("SQL lexer output:");
    let tokens = Lexer::new(sql).tokenize()?;
    for token in &tokens {
        // `:?` uses Rust's Debug formatting, which shows enum variant names
        // and their attached data such as Identifier("name").
        println!("{token:?}");
    }
    let ast = Parser::new(tokens).parse()?;
    println!("\nSQL parser output:\n{ast:#?}");


    let mut db = Database::new();

    let users_schema = Schema::new(vec![
        Column::identity("id"),
        Column::required("name", DataType::Text),
        Column::required("active", DataType::Boolean),
    ])?;

    db.create_table("users", users_schema)?;

    db.insert(
        "users",
        vec![
            Value::Null,
            Value::text("Liam"),
            Value::Boolean(true),
        ],
    )?;

    db.insert(
        "users",
        vec![
            Value::Null,
            Value::text("Alice"),
            Value::Boolean(false),
        ],
    )?;

    println!("All users:");
    print_users(db.select_all("users")?);

    println!("\nActive users:");
    let active_users =
        db.select_where("users", |row| row[2] == Value::Boolean(true))?;
    print_users(&active_users);

    let updated = db.update_where(
        "users",
        |row| row[0] == Value::Integer(2),
        "active",
        Value::Boolean(true),
    )?;
    println!("\nUpdated {updated} row(s).");

    let deleted =
        db.delete_where("users", |row| row[0] == Value::Integer(1))?;
    println!("Deleted {deleted} row(s).");

    println!("\nFinal users:");
    print_users(db.select_all("users")?);

    Ok(())
}

fn print_users<T>(rows: &[T])
where
    T: std::borrow::Borrow<LiDB::Row>,
{
    println!("+----+-------+--------+");
    println!("| id | name  | active |");
    println!("+----+-------+--------+");

    for row in rows {
        let row = row.borrow();
        println!("| {:<2} | {:<5} | {:<6} |", row[0], row[1], row[2]);
    }

    println!("+----+-------+--------+");
}
