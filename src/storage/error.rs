use super::value::DataType; 
use std::error::Error; 
use std::fmt; 

#[derive(Debug,Clone,PartialEq, Eq)]
pub enum DbError {
    TableAlreadyExists {
        table: String,
    },
    TableNotFound{
        table: String,
    },
    ColumnNotFound{
        table: String, 
        column: String,
    },
    DuplicateColumn{
        column: String,
    },
    WrongColumnCount{
        table: String, 
        expected: usize, 
        actual: usize,
    },
    TypeMismatch {
        table: String, 
        column: String, 
        expected: DataType, 
        actual: DataType,
    },
    NullNotAllowed{
        table: String, 
        column: String,
    },
    IdentityOverflow {
        table: String,
    },
    Io(String),
    Corrupt(String),
    Encoding(String),
    PageFull,
    RecordTooLarge {
        size: usize,
        maximum: usize,
    },
    CatalogFull,
}

impl fmt::Display for DbError {
    fn fmt(&self, f: &mut fmt::Formatter<'_> ) -> fmt::Result { 
        match self {
            DbError::TableAlreadyExists { table } => {
                write!(f, "table '{table}' already exists")
            }
            DbError::TableNotFound { table } => {
                write!(f, "table '{table}' does not exist")
            }
            DbError::ColumnNotFound { table, column } => {
                write!(f, "column '{column}' does not exist in table '{table}'")
            }
            DbError::DuplicateColumn { column } => {
                write!(f, "column '{column}' is defined more than once")
            }
            DbError::WrongColumnCount {
                table,
                expected,
                actual,
            } => {
                write!(
                    f,
                    "table '{table}' expects {expected} values, but received {actual}"
                )
            }
            DbError::TypeMismatch {
                table,
                column,
                expected,
                actual,
            } => {
                write!(
                    f,
                    "column '{column}' in table '{table}' expects {expected}, but received {actual}"
                )
            }
            DbError::NullNotAllowed { table, column } => {
                write!(
                    f,
                    "column '{column}' in table '{table}' does not allow NULL"
                )
            }
            DbError::IdentityOverflow { table } => {
                write!(f, "identity column in table '{table}' has no remaining values")
            }
            DbError::Io(message) => write!(f, "I/O error: {message}"),
            DbError::Corrupt(message) => write!(f, "corrupt database: {message}"),
            DbError::Encoding(message) => write!(f, "cannot encode value: {message}"),
            DbError::PageFull => write!(f, "page has no room for this record"),
            DbError::RecordTooLarge { size, maximum } => write!(
                f,
                "record is {size} bytes, but a page can hold at most {maximum}"
            ),
            DbError::CatalogFull => write!(f, "the single-page system catalog is full"),
        
        }
    }
}

impl Error for DbError {}

impl From<std::io::Error> for DbError {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error.to_string())
    }
}
