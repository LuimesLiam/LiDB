use crate::value::Value; 
use std::ops::{Index, IndexMut};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Row {
    values: Vec<Value>,
}

impl Row {
    pub fn new(values: Vec<Value>) -> Self{
        Self { values }
    }

    pub fn values(&self) -> &[Value] {
        &self.values
    }

    pub fn len (&self) -> usize {
        self.values.len()
    }

    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    pub fn get(&self, index:usize) -> Option<&Value> {
        self.values.get(index)
    }

    pub fn get_mut(&mut self, index:usize) ->Option<&mut Value>{
        self.values.get_mut(index)
    }

}


impl Index<usize> for Row {
    type Output = Value; 
    fn index(&self, index:usize) -> &Self::Output {
        &self.values[index]
    }
}

impl IndexMut<usize> for Row {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        &mut self.values[index]
    }
    
}



#[cfg(test)]

mod tests{
    use super::*;

    #[test]
    fn create_new_row(){
        let values = vec![Value::Integer(1), Value::text("Hello"), Value::Boolean(true), Value::Null]; 

        let row = Row::new(values);

        assert_eq!(row[0], Value::Integer(1));
        assert_eq!(row.len(), 4); 
        assert_eq!(row.is_empty(), false);
        assert_eq!(row.get(0), Some(&Value::Integer(1))); 
    }
}