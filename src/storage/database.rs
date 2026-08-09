
use super::error::DbError;
use super::row::Row;
use super::schema::Schema;
use super::table::Table;
use super::value::Value;
use std::collections::HashMap;

#[derive(Debug,Default)]
pub struct Database {
    tables: HashMap<String, Table>, 
}

impl Database{
    pub fn new() -> Self {
        Self::default()
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

        self.tables.insert(name.clone(), Table::new(name, schema));

        Ok(())
    }

    pub fn table_count(&self) -> usize {
        self.tables.len()
    }

    pub fn drop_table(&mut self, name: &str) -> Result<Table, DbError> {
        self.tables.remove(name)
            .ok_or_else(|| DbError::TableNotFound { table: name.to_string() })

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

    pub fn insert(&mut self, table_name: &str, values: Vec<Value>) -> Result<usize, DbError> {
        self.table_mut(table_name)?.insert(Row::new(values))
    }

    pub fn select_all(&self, table_name: &str) -> Result<&[Row], DbError> {
        Ok(self.table(table_name)?.rows())
    }

    pub fn select_where<F>(
        &self,
        table_name: &str,
        predicate: F,
    ) -> Result<Vec<&Row>, DbError>
    where 
        F: Fn(&Row) -> bool
    {
        Ok(self.table(table_name)?.filter(predicate))
    }

    pub fn update_where<F>(
        &mut self, 
        table_name: &str, 
        predicate: F, 
        column_name: &str, 
        new_value: Value
    ) -> Result<usize, DbError>
    where 
        F: Fn(&Row) -> bool
    {
        self.table_mut(table_name)?.update_where(predicate, column_name, new_value)
    }

    pub fn delete_where<F>(
        &mut self, 
        table_name: &str,
        predicate: F
    ) -> Result<usize, DbError>
    where F: Fn(&Row) -> bool
    {
        Ok(self.table_mut(table_name)?.delete_where(predicate))
    }

    pub fn execute(&mut self, sql: &str) -> Result<Vec<Row>, crate::DatabaseError> {
        crate::executor::execute_sql(self, sql)
    }

    pub fn execute_batch(&mut self, sql: &str) -> Result<Vec<Vec<Row>>, crate::DatabaseError> {
        crate::executor::execute_sql_batch(self, sql)
    }

}


#[cfg(test)]

mod tests{
    use super::*;
    use crate::storage::schema::Column;
    use crate::storage::value::DataType;

    fn user_schema() -> Schema {
        Schema::new(vec![
            Column::required("id", DataType::Integer),
            Column::required("name", DataType::Text)
        ]).expect("error creating Schema")
    }
    #[test]
    fn creating_and_dropping_a_table_changes_table_count() {
        let mut db = Database::new();

        db.create_table("users", user_schema()).unwrap();
        assert_eq!(db.table_count(), 1);

        db.drop_table("users").unwrap();
        assert_eq!(db.table_count(), 0);
    }

    #[test]
    fn duplicate_table_names_are_rejected() {
        let mut db = Database::new();

        db.create_table("users", user_schema()).unwrap();
        let result = db.create_table("users", user_schema());

        assert_eq!(
            result,
            Err(DbError::TableAlreadyExists {
                table: "users".to_string(),
            })
        );
    }

    #[test]
    fn unknown_tables_return_an_error() {
        let db = Database::new();

        assert!(matches!(
            db.table("missing"),
            Err(DbError::TableNotFound { .. })
        ));
    }
}




