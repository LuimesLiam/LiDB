use super::error::DbError;
use super::page::{PAGE_SIZE, PageId, PageKind, SlottedPage};
use std::fs::{File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::Path;

const DB_MAGIC: &[u8; 8] = b"TINYREL\0";
const FORMAT_VERSION: u16 = 1;

/// The pager is the only component that knows byte offsets in the database
/// file. Higher layers always address storage using PageId.
pub(crate) struct Pager {
    file: File,
    page_count: u64,
}


impl Pager {
    /// Create a new database file with its metadata page and empty catalog.
    /// Existing files are deliberately not overwritten.
    pub(crate) fn create(path: &Path) -> Result<Self, DbError> {
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .open(path)?;
        let mut pager = Self {
            file,
            // Page 0 is reserved for database metadata.
            page_count: 1,
        };
        pager.write_metadata()?;
        let (catalog_id, _) = pager.allocate(PageKind::Catalog)?;
        debug_assert_eq!(catalog_id, PageId(1));
        pager.flush()?;
        Ok(pager)
    }

    pub(crate) fn open(path: &Path) -> Result<Self, DbError> {
        let mut file = OpenOptions::new().read(true).write(true).open(path)?;
        let length = file.metadata()?.len(); 
        if length < (2* PAGE_SIZE) as u64 || length % PAGE_SIZE as u64 != 0 {
            return Err(DbError::Corrupt("file len is not a whole number of pages".into()));
        }

        let mut header = [0_u8; PAGE_SIZE];
        file.read_exact(&mut header)?; 
        if &header[0..8] != DB_MAGIC {
            return Err(DbError::Corrupt("databse magic number does not match".into()));
        }

        let version = u16::from_le_bytes(header[8..10].try_into().unwrap());
        let page_size = u16::from_le_bytes(header[10..12].try_into().unwrap()) as usize;
        let page_count = u64::from_le_bytes(header[12..20].try_into().unwrap());
        if version != FORMAT_VERSION
            || page_size != PAGE_SIZE
            || page_count * PAGE_SIZE as u64 != length
        {
            return Err(DbError::Corrupt(
                "unsupported or inconsistent database header".into(),
            ));
        }
        Ok(Self { file, page_count })

    }
    pub(crate) fn allocate(&mut self, kind: PageKind) -> Result<(PageId, SlottedPage), DbError> {
        let id = PageId(self.page_count);
        self.page_count += 1;
        let page = SlottedPage::new(kind);
        self.write_page(id, &page)?;
        self.write_metadata()?;
        Ok((id, page))
    }

    pub(crate) fn write_page(&mut self, id: PageId, page: &SlottedPage) -> Result<(), DbError> {
        self.file.seek(SeekFrom::Start(id.0 * PAGE_SIZE as u64))?;
        self.file.write_all(page.bytes())?;
        Ok(())
    }

    
    pub(crate) fn read_page(&mut self, id: PageId, kind: PageKind) -> Result<SlottedPage, DbError> {
        if id.0 >= self.page_count {
            return Err(DbError::Corrupt(format!(
                "page {} is outside the file",
                id.0
            )));
        }
        let mut bytes = [0_u8; PAGE_SIZE];
        self.file.seek(SeekFrom::Start(id.0 * PAGE_SIZE as u64))?;
        self.file.read_exact(&mut bytes)?;
        SlottedPage::from_bytes(bytes, kind)
    }

    pub(crate) fn flush(&mut self) -> Result<(), DbError> {
        self.file.sync_all()?;
        Ok(())
    }

    fn write_metadata(&mut self) -> Result<(), DbError> {
        let mut header = [0_u8; PAGE_SIZE];
        header[0..8].copy_from_slice(DB_MAGIC);
        header[8..10].copy_from_slice(&FORMAT_VERSION.to_le_bytes());
        header[10..12].copy_from_slice(&(PAGE_SIZE as u16).to_le_bytes());
        header[12..20].copy_from_slice(&self.page_count.to_le_bytes());
        self.file.seek(SeekFrom::Start(0))?;
        self.file.write_all(&header)?;
        self.file.set_len(self.page_count * PAGE_SIZE as u64)?;
        Ok(())
    }

}
