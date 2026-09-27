// FILE: src/storage/pager.rs, replace the existing file contents.
use super::buffer_pool::BufferPool;
use super::disk_manager::{DiskManager, FileDiskManager};
use super::error::DbError;
use super::page::{PAGE_SIZE, PageId, PageKind, SlottedPage};
use std::path::Path;

const DB_MAGIC: &[u8; 8] = b"TINYREL\0";
const FORMAT_VERSION: u16 = 1;
// Keep capacity in one place so create/open always build equivalent pools.
const DEFAULT_BUFFER_POOL_FRAMES: usize = 64;

/// Understands LiDB page formats, but never performs raw file I/O itself.
pub(crate) struct Pager {
    buffer_pool: BufferPool<FileDiskManager>,
    page_count: u64,
}

impl Pager {
    /// Create page 0 (metadata) and page 1 (the empty catalog).
    pub(crate) fn create(path: &Path) -> Result<Self, DbError> {
        let mut disk = FileDiskManager::create(path)?;

        let metadata_id = disk.allocate_page()?;
        debug_assert_eq!(metadata_id, PageId(0));

        // Ownership of the disk manager moves into the cache here. From now
        // on Pager must perform page I/O through buffer_pool, not `disk`.
        let mut pager = Self {
            buffer_pool: BufferPool::new(disk, DEFAULT_BUFFER_POOL_FRAMES),
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
        let page_size = u16::from_le_bytes(header[10..12].try_into().unwrap()) as usize;
        let metadata_page_count = u64::from_le_bytes(header[12..20].try_into().unwrap());

        if version != FORMAT_VERSION
            || page_size != PAGE_SIZE
            || metadata_page_count != physical_page_count
        {
            return Err(DbError::Corrupt(
                "unsupported or inconsistent database header".into(),
            ));
        }

        // Header validation above intentionally uses direct startup I/O;
        // steady-state operations begin through this newly-created cache.
        Ok(Self {
            buffer_pool: BufferPool::new(disk, DEFAULT_BUFFER_POOL_FRAMES),
            page_count: physical_page_count,
        })
    }

    pub(crate) fn allocate(&mut self, kind: PageKind) -> Result<(PageId, SlottedPage), DbError> {
        let id = self.buffer_pool.allocate_page()?;
        debug_assert_eq!(id, PageId(self.page_count));

        self.page_count += 1;
        let page = SlottedPage::new(kind);
        self.write_page(id, &page)?;
        self.write_metadata()?;
        Ok((id, page))
    }

    pub(crate) fn write_page(&mut self, id: PageId, page: &SlottedPage) -> Result<(), DbError> {
        // Fetch pins this frame. Copy bytes while the mutable frame borrow is
        // active, then unpin and mark dirty so writeback happens later.
        let frame = self.buffer_pool.fetch_page(id)?;
        frame.data.copy_from_slice(page.bytes());
        self.buffer_pool.unpin_page(id, true)
    }

    pub(crate) fn read_page(&mut self, id: PageId, kind: PageKind) -> Result<SlottedPage, DbError> {
        // Arrays of bytes implement `Copy`, so this makes an owned snapshot.
        // The frame can then be unpinned before page-format validation runs.
        let bytes = self.buffer_pool.fetch_page(id)?.data;
        self.buffer_pool.unpin_page(id, false)?;
        SlottedPage::from_bytes(bytes, kind)
    }

    pub(crate) fn flush(&mut self) -> Result<(), DbError> {
        // Database::close calls this so storage errors can be returned.
        self.buffer_pool.flush_all()
    }

    fn write_metadata(&mut self) -> Result<(), DbError> {
        let mut header = [0_u8; PAGE_SIZE];
        header[0..8].copy_from_slice(DB_MAGIC);
        header[8..10].copy_from_slice(&FORMAT_VERSION.to_le_bytes());
        header[10..12].copy_from_slice(&(PAGE_SIZE as u16).to_le_bytes());
        header[12..20].copy_from_slice(&self.page_count.to_le_bytes());
        let frame = self.buffer_pool.fetch_page(PageId(0))?;
        frame.data.copy_from_slice(&header);
        self.buffer_pool.unpin_page(PageId(0), true)
    }
}
