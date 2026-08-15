use crate::storage::page::PageId;

use super::error::DbError;
use super::row::Row;
use super::schema::Schema;
use super::value::Value;


#[derive(Debug, Clone)]
pub struct Table {
    name: String, 
    schema: Schema, 
    first_page: Option<PageId>,
    row_count: u64,
}


impl Table {
    pub fn new(name: impl Into<String> , schema: Schema) -> Self{
        Self {
            name: name.into(),
            schema: schema, 
            first_page: None, 
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
    pub (crate) fn first_page(&self) -> Option<PageId> {
        self.first_page
    }
    pub (crate) fn set_storage(&mut self, first_page: Option<PageId>, row_count: u64) {
        self.first_page = first_page; 
        self.row_count = row_count; 
    } 


}
