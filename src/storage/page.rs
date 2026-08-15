use super::error::DbError; 


pub const PAGE_SIZE: usize = 4096; 

const PAGE_MAGIC: &[u8; 4] = b"TRPG"; 
const HEADER_SIZE: usize = 24; 
const SLOT_SIZE: usize = 4; 
const NO_PAGE: u64 = u64::MAX; 

/// The stable address of a page in a database file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct PageId(pub u64);

/// A stable address for a row while its page is not rewritten.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RecordId {
    pub page_id: PageId,
    pub slot_id: u16,
}

/// A slot is an indirection entry between a record ID and variable-size bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Slot {
    pub offset: u16,
    pub length: u16,
}

/// Page 1 stores catalog records; data pages store serialized rows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PageKind {
    Catalog = 1,
    Data = 2,
}


#[derive(Clone)]
pub(crate) struct SlottedPage {
    bytes: [u8; PAGE_SIZE],
}


impl SlottedPage { 
    // Create an empty page with the slot table and record area touching.
    pub(crate) fn new(kind: PageKind) -> Self {
        let mut page = Self{
            bytes: [0; PAGE_SIZE],
        };

        page.bytes[0..4].copy_from_slice(PAGE_MAGIC);
        page.bytes[4] = kind as u8; 
        page.set_slot_count(0);
        page.set_free_start(HEADER_SIZE as u16);
        page.set_free_end(PAGE_SIZE as u16);
        page.set_next_page(None);
        page
    }

    // Validate the page header and the boundaries stored in the page before
    // allowing the raw bytes to be used.
    pub(crate) fn from_bytes(bytes: [u8; PAGE_SIZE], expected: PageKind) -> Result<Self, DbError> {
        if &bytes[0..4] != PAGE_MAGIC || bytes[4] != expected as u8 {
            return Err(DbError::Corrupt(format!("expected a {expected:?} slotted page")));
        }

        let page = Self {bytes}; 
        let start = page.free_start() as usize; 
        let end=page.free_end() as usize; 
        let slots_end = HEADER_SIZE + page.slot_count() as usize * SLOT_SIZE; 
        if start != slots_end || start > end || end >PAGE_SIZE {
            return Err(DbError::Corrupt("Invalid slotted-page boundaries".into()))
        }

        Ok(page)
    }

    // Add a record to the page and return the slot ID used to find it later.
    pub(crate) fn insert(&mut self, record: &[u8]) -> Result<u16, DbError> {
        if record.len() > u16::MAX as usize || record.len() + HEADER_SIZE + SLOT_SIZE > PAGE_SIZE {
            return Err(DbError::RecordTooLarge {
                size: record.len(),
                maximum: PAGE_SIZE - HEADER_SIZE - SLOT_SIZE,
            });
        }

        // Reuse a deleted slot when possible. Otherwise the slot array grows.
        let reusable = (0..self.slot_count()).find(|&id| self.slot(id).unwrap().length == 0);
        let needs_slot = reusable.is_none();
        let required = record.len() + if needs_slot { SLOT_SIZE } else { 0 };
        if self.contiguous_free() < required {
            self.compact();
        }
        if self.contiguous_free() < required {
            return Err(DbError::PageFull);
        }

        let slot_id = reusable.unwrap_or_else(|| {
            let id = self.slot_count();
            self.set_slot_count(id + 1);
            self.set_free_start(self.free_start() + SLOT_SIZE as u16);
            id
        });
        let offset = self.free_end() as usize - record.len();
        self.bytes[offset..offset + record.len()].copy_from_slice(record);
        self.set_free_end(offset as u16);
        self.set_slot(
            slot_id,
            Slot {
                offset: offset as u16,
                length: record.len() as u16,
            },
        );
        Ok(slot_id)
    }

    // Look up a record through its slot, returning None for missing or deleted slots.
    pub(crate) fn get(&self, slot_id: u16) -> Result<Option<&[u8]>, DbError> {
        let Some(slot) = self.slot(slot_id) else {
            return Ok(None);
        };
        if slot.length == 0 {
            return Ok(None);
        }
        let start = slot.offset as usize;
        let end = start + slot.length as usize;
        if start < self.free_end() as usize || end > PAGE_SIZE {
            return Err(DbError::Corrupt("slot points outside record area".into()));
        }
        Ok(Some(&self.bytes[start..end]))
    }

    // Return all live records together with their stable slot IDs.
    pub(crate) fn records(&self) -> Result<Vec<(u16, &[u8])>, DbError> {
        let mut records = Vec::new();
        for slot_id in 0..self.slot_count() {
            if let Some(record) = self.get(slot_id)? {
                records.push((slot_id, record));
            }
        }
        Ok(records)
    }

    // Pack live records at the end of the page so the free space is contiguous.
    fn compact(&mut self) {
        let live = (0..self.slot_count())
            .filter_map(|id| {
                let slot = self.slot(id)?;
                (slot.length != 0).then(|| {
                    (
                        id,
                        self.bytes[slot.offset as usize..][..slot.length as usize].to_vec(),
                    )
                })
            })
            .collect::<Vec<_>>();
        let mut end = PAGE_SIZE;
        for (slot_id, record) in live {
            end -= record.len();
            self.bytes[end..end + record.len()].copy_from_slice(&record);
            self.set_slot(
                slot_id,
                Slot {
                    offset: end as u16,
                    length: record.len() as u16,
                },
            );
        }
        self.set_free_end(end as u16);
    }


    fn slot(&self, slot_id: u16) -> Option<Slot> {
        // Deleted slots remain in the table, so an out-of-range ID is the only
        // case where there is no slot entry at all.
        if slot_id >= self.slot_count() {
            return None;
        }
        let at = HEADER_SIZE + slot_id as usize * SLOT_SIZE;
        Some(Slot {
            offset: u16::from_le_bytes(self.bytes[at..at + 2].try_into().unwrap()),
            length: u16::from_le_bytes(self.bytes[at + 2..at + 4].try_into().unwrap()),
        })
    }
    fn set_slot(&mut self, slot_id: u16, slot: Slot) {
        // Slots start after the 24-byte header, and each slot uses 4 bytes:
        // 2 for the record offset and 2 for its length.
        let at = HEADER_SIZE + slot_id as usize * SLOT_SIZE;
        // Keep the fields in little-endian order, like the rest of the page.
        self.bytes[at..at + 2].copy_from_slice(&slot.offset.to_le_bytes());
        self.bytes[at + 2..at + 4].copy_from_slice(&slot.length.to_le_bytes());
    }

    // Read the number of entries currently in the slot table.
    pub(crate) fn slot_count(&self) -> u16 {
        u16::from_le_bytes(self.bytes[6..8].try_into().unwrap())
    }

    // Expose the complete page for writing it back to disk.
    pub(crate) fn bytes(&self) -> &[u8; PAGE_SIZE] {
        &self.bytes
    }

    // Read the page ID of the next page in a linked page chain, if present.
    pub(crate) fn next_page(&self) -> Option<PageId> {
        let value = u64::from_le_bytes(self.bytes[12..20].try_into().unwrap());
        (value != NO_PAGE).then_some(PageId(value))
    }

    // Store the next page ID, or the all-ones sentinel when this is the last page.
    pub(crate) fn set_next_page(&mut self, page_id: Option<PageId>) {
        let value = page_id.map_or(NO_PAGE, |id| id.0);
        self.bytes[12..20].copy_from_slice(&value.to_le_bytes());
    }
    fn contiguous_free(&self) -> usize {
        // The free area is the gap between the slot directory and the records.
        (self.free_end() - self.free_start()) as usize
    }
    fn free_start(&self) -> u16 {
        // Header bytes 8..10 contain the first free byte after the slot table.
        u16::from_le_bytes(self.bytes[8..10].try_into().unwrap())
    }
    fn set_free_start(&mut self, value: u16) {
        // Store the start of the free area in the header.
        self.bytes[8..10].copy_from_slice(&value.to_le_bytes());
    }
    fn free_end(&self) -> u16 {
        // Header bytes 10..12 contain the end of the free area, just before
        // records that grow backward from the end of the page.
        u16::from_le_bytes(self.bytes[10..12].try_into().unwrap())
    }
    fn set_free_end(&mut self, value: u16) {
        // Store the end of the free area in the header.
        self.bytes[10..12].copy_from_slice(&value.to_le_bytes());
    }
    fn set_slot_count(&mut self, value: u16) {
        // Header bytes 6..8 store the number of 4-byte entries in the slot table.
        self.bytes[6..8].copy_from_slice(&value.to_le_bytes());
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn variable_records_are_addressed_by_slots() {
        let mut page = SlottedPage::new(PageKind::Data);
        let first = page.insert(b"short").unwrap();
        let second = page.insert(b"a longer record").unwrap();
        assert_eq!(page.get(first).unwrap(), Some(&b"short"[..]));
        assert_eq!(page.get(second).unwrap(), Some(&b"a longer record"[..]));
        assert!(page.free_start() < page.free_end());
    }
}
