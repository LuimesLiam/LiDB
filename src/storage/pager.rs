use super::disk_manager::{DiskManager, FileDiskManager};
use super::error::DbError;
use super::page::{PAGE_SIZE, PageId, PageKind, SlottedPage};
use std::path::Path;

const DB_MAGIC: &[u8; 8] = b"TINYREL\0";
const FORMAT_VERSION: u16 = 1;

/// Understands LiDB page formats, but never performs raw file I/O itself.
pub(crate) struct Pager {
    disk: FileDiskManager,
    page_count: u64,
}

impl Pager {
    /// Create page 0 (metadata) and page 1 (the empty catalog).
    pub(crate) fn create(path: &Path) -> Result<Self, DbError> {
        let mut disk = FileDiskManager::create(path)?;

        let metadata_id = disk.allocate_page()?;
        debug_assert_eq!(metadata_id, PageId(0));

        let mut pager = Self {
            disk,
            page_count: 1,
        };
        pager.write_metadata()?;

        let (catalog_id, _) = pager.allocate(PageKind::Catalog)?;
        debug_assert_eq!(catalog_id, PageId(1));
        pager.flush()?;
        Ok(pager)
    }

    pub(crate) fn open(path: &Path) -> Result<Self, DbError> {
        let mut disk = FileDiskManager::open(path)?;
        let physical_page_count = disk.page_count();

        if physical_page_count < 2 {
            return Err(DbError::Corrupt(
                "database must contain metadata and catalog pages".into(),
            ));
        }

        let header = disk.read_page(PageId(0))?;
        if &header[0..8] != DB_MAGIC {
            return Err(DbError::Corrupt(
                "database magic number does not match".into(),
            ));
        }

        let version = u16::from_le_bytes(header[8..10].try_into().unwrap());
        let page_size =
            u16::from_le_bytes(header[10..12].try_into().unwrap()) as usize;
        let metadata_page_count =
            u64::from_le_bytes(header[12..20].try_into().unwrap());

        if version != FORMAT_VERSION
            || page_size != PAGE_SIZE
            || metadata_page_count != physical_page_count
        {
            return Err(DbError::Corrupt(
                "unsupported or inconsistent database header".into(),
            ));
        }

        Ok(Self {
            disk,
            page_count: physical_page_count,
        })
    }

    pub(crate) fn allocate(
        &mut self,
        kind: PageKind,
    ) -> Result<(PageId, SlottedPage), DbError> {
        let id = self.disk.allocate_page()?;
        debug_assert_eq!(id, PageId(self.page_count));

        self.page_count += 1;
        let page = SlottedPage::new(kind);
        self.disk.write_page(id, page.bytes())?;
        self.write_metadata()?;
        Ok((id, page))
    }

    pub(crate) fn write_page(
        &mut self,
        id: PageId,
        page: &SlottedPage,
    ) -> Result<(), DbError> {
        self.disk.write_page(id, page.bytes())
    }

    pub(crate) fn read_page(
        &mut self,
        id: PageId,
        kind: PageKind,
    ) -> Result<SlottedPage, DbError> {
        let bytes = self.disk.read_page(id)?;
        SlottedPage::from_bytes(bytes, kind)
    }

    pub(crate) fn flush(&mut self) -> Result<(), DbError> {
        self.disk.sync_all()
    }

    fn write_metadata(&mut self) -> Result<(), DbError> {
        let mut header = [0_u8; PAGE_SIZE];
        header[0..8].copy_from_slice(DB_MAGIC);
        header[8..10].copy_from_slice(&FORMAT_VERSION.to_le_bytes());
        header[10..12].copy_from_slice(&(PAGE_SIZE as u16).to_le_bytes());
        header[12..20].copy_from_slice(&self.page_count.to_le_bytes());
        self.disk.write_page(PageId(0), &header)
    }
}