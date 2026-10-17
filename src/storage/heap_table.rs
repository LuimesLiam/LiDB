use crate::Slot;

use super::error::DbError;
use super::page::{PageId, PageKind, RecordId, SlottedPage};
use super::pager::Pager;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HeapTable {
    pub first_page_id: PageId,
    pub last_page_id: PageId,
}

impl HeapTable {
    pub(crate) fn create(
        pager: &mut Pager, 
        bytes: &[u8],
    ) -> Result<(Self,RecordId), DbError> {
        let mut page = SlottedPage::new(PageKind::Data); // create local page 
        let slot_id = page.insert(bytes)?; // insert bytes into local page
        let (page_id , _) = pager.allocate(PageKind::Data)?; // allocate a new page in the database
        pager.write_page(page_id, &page)?;

        let heap = Self {
            first_page_id: page_id,
            last_page_id: page_id,
        };
        Ok((heap, RecordId { page_id, slot_id }))
    }

    pub(crate) fn insert(&mut self, pager: &mut Pager, bytes: &[u8],) -> Result<RecordId, DbError> {
        //read last page id and confirm no successors 
        let mut tail = pager.read_page(self.last_page_id, PageKind::Data)?;
    
        if tail.next_page().is_some() {
            return Err(DbError::Corrupt("heap table last page has a successor".into()));
        }

        //try and insert 
        match tail.insert(bytes) {
            Ok(slot_id) => {
                pager.write_page(self.last_page_id, &tail)?;
                Ok(RecordId { page_id: self.last_page_id, slot_id })
            }
            Err(DbError::PageFull) => {
                let mut new_page = SlottedPage::new(PageKind::Data);
                let slot_id = new_page.insert(bytes)?;

                new_page.set_previous_page(Some(self.last_page_id));
                
                let (new_id, _) = pager.allocate(PageKind::Data)?;
                pager.write_page(new_id, &new_page)?;

                tail.set_next_page(Some(new_id));
                pager.write_page(self.last_page_id, &tail)?; // update the old tail page to point to the new page

                self.last_page_id = new_id;
                Ok(RecordId { page_id: new_id, slot_id })
            }
            Err(e) => Err(e)
        }
    }
    
    pub fn new(first_page_id: PageId, last_page_id: PageId) -> Self {
        Self { first_page_id, last_page_id }
    }
}