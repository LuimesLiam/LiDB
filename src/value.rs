use std::fmt; 

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DataType {
    Integer, 
    Boolean, 
    Text
}

impl fmt::Display for DataType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DataType::Integer => write!(f, "INTEGER"),
            DataType::Boolean => write!(f, "BOOLEAN"), 
            DataType::Text    => write!(f, "TEXT"),
        }
    }
}

//// A single valuue stored in the DB
/// A cell can be these below defined value types 
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Value {
    Null, 
    Integer(i64),
    Boolean(bool), 
    Text(String)
}

impl Value {
    pub fn data_type(&self) -> Option<DataType> {
        match self {
            Value::Null => None, 
            Value::Integer(_) => Some(DataType::Integer), 
            Value::Boolean(_) => Some(DataType::Boolean),
            Value::Text(_) => Some(DataType::Text)
        }
    }

    pub fn text(value: impl Into<String>) -> Self {
        Value::Text(value.into())
    }
}


impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self{
            Value::Null           => write!(f, "NULL"),
            Value::Integer(value) => write!(f,"{value}"),
            Value::Boolean(value) => write!(f,"{value}"),
            Value::Text(value)    => write!(f,"{value}"),
        }
    }
}

#[cfg(test)]
mod tests{
    use super::*;
    #[test]
    fn value_report_its_data_type() {
        assert_eq!(Value::Integer(10).data_type(), Some(DataType::Integer));
        assert_eq!(Value::Boolean(true).data_type(), Some(DataType::Boolean));
        assert_eq!(Value::text("Hello World").data_type(), Some(DataType::Text));
        assert_eq!(Value::Null.data_type(), None);
    }

    #[test]
    fn text_helper_construct_a_text_value() { 
        assert_eq!(Value::text("Hello"), Value::Text("Hello".to_string()));
    }
}
