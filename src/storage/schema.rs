use super::error::DbError; 
use super::row::Row; 
use super::value::{DataType, Value};
use std::collections::HashSet; 

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Column {
   name: String, 
   data_type: DataType, 
   nullable: bool, 
   identity: bool,
}

impl Column {
    pub fn new(name: impl Into<String> , data_type: DataType, nullable: bool) -> Self{
        Self { name: name.into(), data_type, nullable, identity: false }
    }

    pub fn required(name: impl Into<String> ,data_type: DataType) -> Self {
        Self::new(name, data_type, false)
    }
    
    pub fn nullable(name: impl Into<String>, data_type: DataType) -> Self {
        Self::new(name, data_type, true)
    }

    /// An automatically generated, non-null integer column.
    pub fn identity(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            data_type: DataType::Integer,
            nullable: false,
            identity: true,
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn data_type(&self) -> DataType {
        self.data_type
    }

    pub fn is_nullable(&self) -> bool {
        self.nullable
    }

    pub fn is_identity(&self) -> bool {
        self.identity
    }
}


#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Schema{
    columns: Vec<Column>,
}


impl Schema{
    pub fn new(columns: Vec<Column>) -> Result<Self, DbError> {
        let mut names = HashSet::new(); 

        for column in &columns {
            if !names.insert(column.name().to_string()) {
                return Err(DbError::DuplicateColumn{
                    column: column.name().to_string(),
                });
            }
        }

        Ok(Self {columns})
    }

    pub fn columns(&self) -> &[Column] {
        &self.columns
    }

    pub fn len(&self) -> usize { 
        self.columns.len()
    }

    pub fn is_empty(&self) -> bool {
        self.columns.is_empty()
    }

    pub fn column_index(&self, column_name: &str) -> Option<usize> {
        self.columns
            .iter()
            .position(|column| column.name() == column_name)
    }
    //// Validate row before its inserted 
    pub fn validate_row(&self, table_name: &str, row: &Row) -> Result<(), DbError>{
        if row.len() != self.columns.len() { 
            return Err(DbError::WrongColumnCount {
                table: table_name.to_string(),
                expected: self.columns.len(), 
                actual: row.len()
            });
        }

        for (column, value) in self.columns.iter().zip(row.values()) {
            self.validate_value(table_name, column, value)?;
        }

        Ok(())  
    }

    pub fn validate_value(&self, table_name: &str, column: &Column, value: &Value) -> Result<(), DbError>{
        match value.data_type(){
            None if !column.is_nullable() => Err(DbError::NullNotAllowed { 
                table: table_name.to_string(), 
                column: column.name().to_string() 
            }),
            None => Ok(()),
            Some(actual) if actual != column.data_type() => Err(DbError::TypeMismatch { 
                table: table_name.to_string(), 
                column: column.name().to_string(), 
                expected: column.data_type(), 
                actual 
            }),
            Some(_) => Ok(())
        }
    }
}

#[cfg(test)]

mod tests{
    use super::*;

    #[test]
    fn create_column(){
        let col = Column::new("test", DataType::Integer, false);
        assert_eq!(*col.name, "test".to_string());
        assert_eq!(col.data_type, DataType::Integer);
        assert_eq!(col.is_nullable(), false);
    }
    #[test]
    fn create_required_column(){
        let col = Column::required("test", DataType::Boolean);

        assert_eq!(*col.name, "test".to_string());
        assert_eq!(col.data_type, DataType::Boolean);
        assert_eq!(col.is_nullable(), false);
    }
    #[test]
    fn create_nullable_column(){
        let col = Column::nullable("test", DataType::Text);
        
        assert_eq!(col.data_type, DataType::Text);
        assert_eq!(col.is_nullable(), true);
    }

    fn user_schema() -> Schema {
        Schema::new(vec![
            Column::required("id", DataType::Integer),
            Column::required("name", DataType::Text),
            Column::nullable("active", DataType::Boolean),
        ])
        .unwrap()
    }

    #[test]
    fn schema_rejects_duplicate_column_names() {
        let result = Schema::new(vec![
            Column::required("id", DataType::Integer),
            Column::required("id", DataType::Integer),
        ]);

        assert_eq!(
            result,
            Err(DbError::DuplicateColumn {
                column: "id".to_string()
            })
        );
    }

    #[test]
    fn schema_accepts_a_valid_row() {
        let row = Row::new(vec![
            Value::Integer(1),
            Value::text("Liam"),
            Value::Boolean(true),
        ]);

        assert_eq!(user_schema().validate_row("users", &row), Ok(()));
    }

    #[test]
    fn schema_rejects_wrong_column_count() {
        let row = Row::new(vec![Value::Integer(1)]);

        assert_eq!(
            user_schema().validate_row("users", &row),
            Err(DbError::WrongColumnCount {
                table: "users".to_string(),
                expected: 3,
                actual: 1,
            })
        );
    }

    #[test]
    fn schema_rejects_wrong_value_type() {
        let row = Row::new(vec![
            Value::text("not-an-integer"),
            Value::text("Liam"),
            Value::Boolean(true),
        ]);

        assert_eq!(
            user_schema().validate_row("users", &row),
            Err(DbError::TypeMismatch {
                table: "users".to_string(),
                column: "id".to_string(),
                expected: DataType::Integer,
                actual: DataType::Text,
            })
        );
    }

    #[test]
    fn schema_rejects_null_for_required_column() {
        let row = Row::new(vec![
            Value::Integer(1),
            Value::Null,
            Value::Boolean(true),
        ]);

        assert_eq!(
            user_schema().validate_row("users", &row),
            Err(DbError::NullNotAllowed {
                table: "users".to_string(),
                column: "name".to_string(),
            })
        );
    }

    #[test]
    fn schema_accepts_null_for_nullable_column() {
        let row = Row::new(vec![
            Value::Integer(1),
            Value::text("Liam"),
            Value::Null,
        ]);

        assert_eq!(user_schema().validate_row("users", &row), Ok(()));
    }


}
