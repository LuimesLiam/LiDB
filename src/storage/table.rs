use crate::storage::heap_table::HeapTable;
use super::schema::Schema;


#[derive(Debug, Clone)]
pub struct Table {
    name: String, 
    schema: Schema, 
    heap: Option<HeapTable>,
    row_count: u64,
}


impl Table {
    pub fn new(name: impl Into<String> , schema: Schema) -> Self{
        Self {
            name: name.into(),
            schema: schema, 
            heap: None,
            row_count: 0,
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn schema(&self) -> &Schema {
        &self.schema
    }


    pub fn row_count(&self) -> usize {
        self.row_count as usize
    }
    pub (crate) fn heap(&self) -> Option<HeapTable> {
        self.heap
    }
    pub (crate) fn set_storage(&mut self, heap: Option<HeapTable>, row_count: u64) {
        self.heap = heap;
        self.row_count = row_count; 
    } 


}
