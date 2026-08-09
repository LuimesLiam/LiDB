use LiDB::{Database, Row};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut database = Database::new();

    // Each statement is parsed, bound, and run through the executor pipeline.
    database.execute_batch(
        "
        CREATE TABLE users (id INTEGER, name TEXT, active BOOLEAN);
        INSERT INTO users VALUES (1, 'Liam', true);
        INSERT INTO users VALUES (2, 'Alice', false);
        INSERT INTO users VALUES (3, 'Bob', true);
        ",
    )?;

    println!("Active users:");
    print_table(
        &["id", "name"],
        &database.execute("SELECT id, name FROM users WHERE active = true;")?,
    );

    let updated = database.execute(
        "UPDATE users SET active = true, id = id + 10 WHERE name = 'Alice';",
    )?;
    println!("\nUpdated {} row(s):", updated.len());
    print_table(&["id", "name", "active"], &updated);

    let deleted = database.execute("DELETE FROM users WHERE name = 'Bob';")?;
    println!("\nDeleted {} row(s):", deleted.len());
    print_table(&["id", "name", "active"], &deleted);

    println!("\nFinal users:");
    print_table(
        &["id", "name", "active"],
        &database.execute("SELECT * FROM users;")?,
    );

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
