use super::error::DbError;
use super::row::Row;
use super::schema::Schema;
use super::value::Value;


#[derive(Debug, Clone)]
pub struct Table {
    name: String, 
    schema: Schema, 
    rows: Vec<Row>,
    // This is deliberately independent from `rows.len()`: deleting a row must
    // not cause its identity value to be reused.
    next_identity: i64,
}


impl Table {
    pub fn new(name: impl Into<String> , schema: Schema) -> Self{
        Self {
            name: name.into(),
            schema: schema, 
            rows: Vec::new(),
            next_identity: 1,
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn schema(&self) -> &Schema {
        &self.schema
    }

    pub fn rows(&self) -> &[Row] {
        &self.rows
    }

    pub fn row_count(&self) -> usize {
        self.rows.len()
    }

    pub(crate) fn rows_mut(&mut self) -> &mut [Row] {
        &mut self.rows
    }

    /// Inserts one row after validating it against the table schema.
    ///
    /// Returns the row's current in-memory position. This is useful for Step 1,
    /// but it is not a durable record identifier. A later storage phase will
    /// replace it with a page ID and slot ID.

    pub fn insert(&mut self, mut row: Row) -> Result<usize, DbError> {
        // `NULL` in an identity column means "please generate the value".
        // Do this before validation, because identity columns are non-null.
        if let Some(identity_index) = self
            .schema
            .columns()
            .iter()
            .position(|column| column.is_identity())
        {
            if let Some(Value::Null) = row.get(identity_index) {
                // Identity values are assigned at insert time, after the caller builds the row.
                row[identity_index] = Value::Integer(self.next_identity);
            }
        }

        self.schema.validate_row(&self.name, &row)?;

        if let Some(identity_index) = self
            .schema
            .columns()
            .iter()
            .position(|column| column.is_identity())
        {
            if let Value::Integer(id) = row[identity_index] {
                let next_after_id = id
                    .checked_add(1)
                    .ok_or(DbError::IdentityOverflow { table: self.name.clone() })?;
                // Explicit IDs may jump ahead, so follow the highest value we've seen.
                self.next_identity = self.next_identity.max(next_after_id);
            }
        }

        self.rows.push(row);
        Ok(self.row_count() -1)
    } 

    // Retun ref to every row matching the predicate 

    pub fn filter<F> (&self, predicate: F) -> Vec<&Row> 
    where 
        F: Fn(&Row) -> bool,
    {
        self.rows
            .iter()
            .filter(|row| predicate(*row))
            .collect::<Vec<&Row>>()
    }
    // pub fn filter<F>(&self, predicate: F) -> Vec<&Row>
    // where
    //     F: Fn(&Row) -> bool,
    // {
    //     let mut matching_rows = Vec::new();

    //     for row in &self.rows {
    //         if predicate(row) {
    //             matching_rows.push(row);
    //         }
    //     }

    //     matching_rows
    // }

    pub fn update_where<F>(&mut self, predicate: F, column_name: &str, new_value: Value) -> Result<usize, DbError>
    where 
        F: Fn(&Row) -> bool
    {
        let column_index = 
            self.schema.column_index(column_name)
                .ok_or_else(|| DbError::ColumnNotFound { table: self.name.clone(), column: column_name.to_string() })?;

        let column = &self.schema.columns()[column_index];
        self.schema.validate_value(&self.name.to_string(), column, &new_value)?;

        let mut updated = 0; 

        for row in &mut self.rows {
            if predicate(row) {
                row[column_index] = new_value.clone();
                updated += 1; 
            }
        }

        Ok(updated)
    }

    pub fn delete_where<F> (&mut self, predicate: F) -> usize 
    where 
        F: Fn(&Row) -> bool 
    {
        let old_len = self.row_count(); 
        self.rows.retain(|row| !predicate(row));
        old_len - self.row_count()
    }

    pub(crate) fn replace_rows(&mut self, rows: Vec<Row>) {
        self.rows = rows;
    }

}

#[cfg(test)]
mod tests{
    use super::*;
    use crate::storage::schema::Column; 
    use crate::storage::value::DataType;

    fn user_table() -> Table { 
        let schema = Schema::new( vec![
            Column::required("id",DataType::Integer),
            Column::required("name", DataType::Text),
            Column::nullable("active", DataType::Boolean)
        ])
        .unwrap();
        
        Table::new( "users", schema )

    }
    #[test]
    fn inserting_a_valid_row_returns_its_position() {
        let mut table = user_table();

        let position = table    
            .insert(Row::new(vec![
                Value::Integer(4),
                Value::text("Bob"),
                Value::Boolean(true)
            ]))
            .expect("failed insert row");

        assert_eq!(position, 0);
        assert_eq!(table.row_count(), 1);
    }

    #[test]
    fn identity_column_generates_ids_without_reusing_deleted_ones() {
        let schema = Schema::new(vec![
            Column::identity("id"),
            Column::required("name", DataType::Text),
        ])
        .unwrap();
        let mut table = Table::new("users", schema);

        table.insert(Row::new(vec![Value::Null, Value::text("Liam")])).unwrap();
        table.insert(Row::new(vec![Value::Null, Value::text("Alice")])).unwrap();
        table.delete_where(|row| row[0] == Value::Integer(1));
        table.insert(Row::new(vec![Value::Null, Value::text("Bob")])).unwrap();

        assert_eq!(table.rows()[0][0], Value::Integer(2));
        assert_eq!(table.rows()[1][0], Value::Integer(3));
    }

    #[test]
    fn filter_returns_only_matching_rows() {
        let mut table = user_table();

        table
            .insert(Row::new(vec![
                Value::Integer(1),
                Value::text("Liam"),
                Value::Boolean(true),
            ]))
            .unwrap();

        table
            .insert(Row::new(vec![
                Value::Integer(2),
                Value::text("Alice"),
                Value::Boolean(false),
            ]))
            .unwrap();
        let active_users = table.filter(|row| row[2] == Value::Boolean(true));

        assert_eq!(active_users.len(), 1);
        assert_eq!(active_users[0][1],Value::text("Liam"));
    }

    
    #[test]
    fn update_changes_only_matching_rows() {
        let mut table = user_table();

        for (id, name) in [(1, "Liam"), (2, "Alice")] {
            table
                .insert(Row::new(vec![
                    Value::Integer(id),
                    Value::text(name),
                    Value::Boolean(true),
                ]))
                .unwrap();
        }

        let updated = table
            .update_where(
                |row| row[0] == Value::Integer(2),
                "active",
                Value::Boolean(false),
            )
            .unwrap();

        assert_eq!(updated, 1);
        assert_eq!(table.rows()[0][2], Value::Boolean(true));
        assert_eq!(table.rows()[1][2], Value::Boolean(false));
    }

    #[test]
    fn update_validates_before_modifying_rows() {
        let mut table = user_table();

        table
            .insert(Row::new(vec![
                Value::Integer(1),
                Value::text("Liam"),
                Value::Boolean(true),
            ]))
            .unwrap();

        let result = table.update_where(
            |_| true,
            "active",
            Value::text("not-a-boolean"),
        );

        assert!(matches!(result, Err(DbError::TypeMismatch { .. })));
        assert_eq!(table.rows()[0][2], Value::Boolean(true));
    }

    #[test]
    fn delete_removes_matching_rows() {
        let mut table = user_table();

        for (id, name) in [(1, "Liam"), (2, "Alice")] {
            table
                .insert(Row::new(vec![
                    Value::Integer(id),
                    Value::text(name),
                    Value::Boolean(true),
                ]))
                .unwrap();
        }

        let deleted = table.delete_where(|row| row[0] == Value::Integer(1));

        assert_eq!(deleted, 1);
        assert_eq!(table.row_count(), 1);
        assert_eq!(table.rows()[0][1], Value::text("Alice"));
    }
}
