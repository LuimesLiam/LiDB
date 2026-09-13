use LiDB::{Database, Row};

const DATABASE_PATH: &str = "data/persistent-example.lidb";

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Run this once to create and populate the database. Database::create
    // refuses to overwrite an existing file.
    //create_persistent_database()?;

    //add_new_table()?;
    // On later runs, comment out the call above and uncomment this one.
    open_persistent_database()?;

    Ok(())
}

fn create_persistent_database() -> Result<(), Box<dyn std::error::Error>> {
    let mut database = Database::create(DATABASE_PATH)?;

    // Each statement is parsed, bound, and run through the executor pipeline.
    database.execute_batch(
        "
        CREATE TABLE users (id INTEGER, name TEXT, active BOOLEAN);
        INSERT INTO users VALUES (1, 'Liam', true);
        INSERT INTO users VALUES (2, 'Alice', false);
        INSERT INTO users VALUES (3, 'Bob', true);
        ",
    )?;

    println!("Users saved to {DATABASE_PATH}:");
    print_table(
        &["id", "name", "active"],
        &database.execute("SELECT * FROM users;")?,
    );

    // close() explicitly flushes the file and lets us handle any I/O error.
    database.close()?;
    Ok(())
}

fn add_new_table() -> Result<(), Box<dyn std::error::Error>> {
    let mut database = Database::open(DATABASE_PATH)?;

    println!("Users loaded from {DATABASE_PATH}:");
    let sql: &'static str = "
        CREATE TABLE products (id INTEGER, name TEXT, price INTEGER); 
        INSERT INTO products VALUES (1, 'Milk', 3);
    ";
    database.execute_batch(sql)?;

    print_table(
        &["id", "name", "price"], 
        &database.execute("SELECT * FROM products;")? 
    );
    database.close(); 
    Ok(())
}

fn open_persistent_database() -> Result<(), Box<dyn std::error::Error>> {
    let mut database = Database::open(DATABASE_PATH)?;

    println!("Users loaded from {DATABASE_PATH}:");
    print_table(
        &["id", "name", "active"],
        &database.execute("SELECT * FROM users;")?,
    );

    print_table(
        &["id", "name", "price"], 
        &database.execute("SELECT * FROM products;")? 
    );

    database.close()?;
    Ok(())
}

fn print_table(headers: &[&str], rows: &[Row]) {
    let mut widths: Vec<usize> = headers.iter().map(|header| header.len()).collect();

    for row in rows {
        for (index, value) in row.values().iter().enumerate() {
            if let Some(width) = widths.get_mut(index) {
                *width = (*width).max(value.to_string().len());
            }
        }
    }

    print_separator(&widths);
    print_cells(headers.iter().copied(), &widths);
    print_separator(&widths);

    for row in rows {
        print_cells(row.values().iter().map(ToString::to_string), &widths);
    }

    print_separator(&widths);
    if rows.is_empty() {
        println!("(no rows)");
    }
}

fn print_separator(widths: &[usize]) {
    print!("+");
    for width in widths {
        print!("{}+", "-".repeat(width + 2));
    }
    println!();
}

fn print_cells<I>(cells: I, widths: &[usize])
where
    I: IntoIterator,
    I::Item: AsRef<str>,
{
    print!("|");
    for (cell, width) in cells.into_iter().zip(widths) {
        print!(" {:<width$} |", cell.as_ref(), width = width);
    }
    println!();
}
