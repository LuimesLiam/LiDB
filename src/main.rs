use LiDB::{Column, DataType, Database, Schema, Value};

fn main() -> Result<(), Box<dyn std::error::Error>> {
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
