use super::error::DbError;
use super::page::{PAGE_SIZE, PageId};
use std::fs::{File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::Path;

pub trait DiskManager {
    fn read_page(&mut self, page_id: PageId) -> Result<[u8; PAGE_SIZE], DbError>;

    fn write_page(&mut self, page_id: PageId, data: &[u8; PAGE_SIZE]) -> Result<(), DbError>;

    fn allocate_page(&mut self) -> Result<PageId, DbError>;
}

pub struct FileDiskManager {
    file: File,
    page_count: u64,
}

impl FileDiskManager {
    pub(crate) fn create(path: &Path) -> Result<Self, DbError> {
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .open(path)?;

        Ok(Self {
            file,
            page_count: 0,
        })
    }

    pub(crate) fn open(path: &Path) -> Result<Self, DbError> {
        let file = OpenOptions::new().read(true).write(true).open(path)?;
        let length = file.metadata()?.len();

        if length % PAGE_SIZE as u64 != 0 {
            return Err(DbError::Corrupt(format! {
                "file length {length} is not a whole number of pages"
            }));
        }

        Ok(Self {
            file,
            page_count: length / PAGE_SIZE as u64,
        })
    }

    pub(crate) fn page_count(&self) -> u64 {
        self.page_count
    }

    pub(crate) fn sync_all(&mut self) -> Result<(), DbError> {
        self.file.sync_all()?;

        Ok(())
    }

    fn offset(page_id: PageId) -> Result<u64, DbError> {
        page_id
            .0
            .checked_mul(PAGE_SIZE as u64)
            .ok_or_else(|| DbError::Corrupt("page offset overflow".into()))
    }

    fn ensure_allocated(&self, page_id: PageId) -> Result<(), DbError> {
        if page_id.0 >= self.page_count {
            return Err(DbError::Corrupt(format!(
                "page {} is outside a file containing {} pages",
                page_id.0, self.page_count
            )));
        }

        Ok(())
    }
}

impl DiskManager for FileDiskManager {
    fn read_page(&mut self, page_id: PageId) -> Result<[u8; PAGE_SIZE], DbError> {
        self.ensure_allocated(page_id)?;

        let mut buffer = [0_u8; PAGE_SIZE];
        self.file.seek(SeekFrom::Start(Self::offset(page_id)?))?;
        self.file.read_exact(&mut buffer)?;
        Ok(buffer)
    }

    fn write_page(&mut self, page_id: PageId, data: &[u8; PAGE_SIZE]) -> Result<(), DbError> {
        self.ensure_allocated(page_id)?;

        self.file.seek(SeekFrom::Start(Self::offset(page_id)?))?;
        self.file.write_all(data)?;
        Ok(())
    }

    fn allocate_page(&mut self) -> Result<PageId, DbError> {
        let page_id = PageId(self.page_count);
        let empty_page = [0_u8; PAGE_SIZE];

        self.file.seek(SeekFrom::End(0))?;
        self.file.write_all(&empty_page)?;
        self.page_count += 1;

        Ok(page_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    static TEST_FILE_NUMBER: AtomicU64 = AtomicU64::new(0);

    fn test_path(test_name: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!(
            "lidb-disk-manager-{test_name}-{}-{}.db",
            std::process::id(),
            TEST_FILE_NUMBER.fetch_add(1, Ordering::Relaxed)
        ))
    }

    #[test]
    fn allocated_pages_are_zero_filled_and_sequential() {
        let path = test_path("allocate");
        let mut disk = FileDiskManager::create(&path).unwrap();

        assert_eq!(disk.allocate_page().unwrap(), PageId(0));
        assert_eq!(disk.allocate_page().unwrap(), PageId(1));
        assert_eq!(disk.page_count(), 2);
        assert_eq!(disk.read_page(PageId(1)).unwrap(), [0_u8; PAGE_SIZE]);
        assert_eq!(
            std::fs::metadata(&path).unwrap().len(),
            (2 * PAGE_SIZE) as u64
        );

        drop(disk);
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn a_full_page_round_trips() {
        let path = test_path("round-trip");
        let mut disk = FileDiskManager::create(&path).unwrap();
        let page_id = disk.allocate_page().unwrap();

        let mut expected = [0_u8; PAGE_SIZE];
        for (index, byte) in expected.iter_mut().enumerate() {
            *byte = (index % 251) as u8;
        }

        disk.write_page(page_id, &expected).unwrap();
        disk.sync_all().unwrap();
        drop(disk);

        let mut reopened = FileDiskManager::open(&path).unwrap();
        assert_eq!(reopened.read_page(page_id).unwrap(), expected);

        drop(reopened);
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn writing_an_unallocated_page_is_rejected() {
        let path = test_path("bounds");
        let mut disk = FileDiskManager::create(&path).unwrap();

        let result = disk.write_page(PageId(10), &[0_u8; PAGE_SIZE]);
        assert!(matches!(result, Err(DbError::Corrupt(_))));

        drop(disk);
        std::fs::remove_file(path).unwrap();
    }
}
