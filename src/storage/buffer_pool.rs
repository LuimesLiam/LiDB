use std::collections::HashMap;

use crate::{DbError, DiskManager, PAGE_SIZE, PageId};

#[derive(Debug)]
pub struct Frame { 
    pub page_id: Option<PageId>, 
    pub data: [u8; PAGE_SIZE],
    pub dirty: bool, 
    pub pin_count: usize,
}


impl Frame {
    fn empty() -> Self {
        Self {
            page_id: None, 
            data: [0; PAGE_SIZE],
            dirty: false, 
            pin_count: 0, 
        }
    }
}

pub struct BufferPool <D: DiskManager> { 
    disk: D, 
    frames: Vec<Frame>, 
    page_table: HashMap<PageId, usize>, 
    last_used: Vec<u64>,
    access_clock: u64
}


impl<D: DiskManager> BufferPool<D> {
    pub fn new (disk:D , capacity: usize) -> Self {
        assert!(capacity> 0, "A buffer pool needs at least oen frame"); 
        Self {
            disk, 
            frames: (0..capacity).map(|_| Frame::empty()).collect(), 
            page_table: HashMap::with_capacity(capacity),
            last_used: vec![0; capacity], 
            access_clock: 0,
        }
    }

    pub fn fetch_page(&mut self, page_id: PageId) -> Result<&mut Frame, DbError> { 
        if let Some(&frame_index) = self.page_table.get(&page_id) {
            self.frames[frame_index].pin_count += 1; 
            self.touch(frame_index);
            return Ok(&mut self.frames[frame_index]);
        }

        let frame_index = self.victim_index().ok_or(DbError::AllFramesPinned)?; 

        if self.frames[frame_index].dirty {
            let old_page_id = self.frames[frame_index]
                .page_id 
                .expect("a dirt frame must contain a page");

            self.disk
                .write_page(old_page_id, &self.frames[frame_index].data)?;
            self.frames[frame_index].dirty = false;
        }

        let requested_data = self.disk.read_page(page_id)?;

        if let Some(old_page_id) = self.frames[frame_index].page_id {
            self.page_table.remove(&old_page_id);
        }

        self.frames[frame_index] = Frame {
            page_id: Some(page_id), 
            data: requested_data, 
            dirty: false, 
            pin_count: 1 
        }; 

        self.page_table.insert(page_id, frame_index);
        self.touch(frame_index);

        Ok(&mut self.frames[frame_index])
    }

    pub fn unpin_page(&mut self, page_id: PageId, dirty: bool) -> Result<(), DbError> { 
        let &frame_index = self.page_table.get(&page_id).ok_or_else(|| {
            DbError::Corrupt(format!("cannot unpin page {}: it is not cached", page_id.0))
        })?;

        let frame = &mut self.frames[frame_index]; 
        if frame.pin_count == 0 {
            return Err(DbError::Corrupt(format!("cannot unpin page {}: its pin count is already zero", page_id.0)));
        }
        frame.pin_count -= 1; 
        frame.dirty |= dirty; 
        Ok(())
    }

    pub fn allocate_page(&mut self) -> Result<PageId, DbError> {
        self.disk.allocate_page()
    }

    pub fn flush_all(&mut self) -> Result<(), DbError> {
        for frame in &mut self.frames {
            if frame.dirty {
                let page_id = frame.page_id.expect("a dirty frame must contain a page");
                self.disk.write_page(page_id, &frame.data)?;
                frame.dirty = false;
            }
        }
        self.disk.sync()
    }
    fn touch(&mut self, frame_index: usize) {
        self.access_clock = self.access_clock.wrapping_add(1); 
        self.last_used[frame_index] = self.access_clock; 
    }
    pub fn shutdown(mut self) -> Result<D, DbError> {
        self.flush_all()?;
        Ok(self.disk)
    }

    fn victim_index(&self) -> Option<usize> { 
        self.frames
        .iter()
        .position(|frame| frame.page_id.is_none())
        .or_else(|| {
            self.frames.iter().enumerate().filter(|(_, frame)| frame.pin_count == 0 )
            .min_by_key(|(index, _)| self.last_used[*index])
            .map(|(index, _)| index )
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::FileDiskManager;
    use std::sync::atomic::{AtomicU64, Ordering};

    static TEST_FILE_NUMBER: AtomicU64 = AtomicU64::new(0);

    struct MemoryDisk {
        pages: Vec<[u8; PAGE_SIZE]>,
    }

    impl DiskManager for MemoryDisk {
        fn read_page(&mut self, page_id: PageId) -> Result<[u8; PAGE_SIZE], DbError> {
            self.pages
                .get(page_id.0 as usize)
                .copied()
                .ok_or_else(|| DbError::Corrupt("test page is out of bounds".into()))
        }

        fn write_page(&mut self, page_id: PageId, data: &[u8; PAGE_SIZE]) -> Result<(), DbError> {
            let page = self
                .pages
                .get_mut(page_id.0 as usize)
                .ok_or_else(|| DbError::Corrupt("test page is out of bounds".into()))?;
            *page = *data;
            Ok(())
        }

        fn allocate_page(&mut self) -> Result<PageId, DbError> {
            let page_id = PageId(self.pages.len() as u64);
            self.pages.push([0; PAGE_SIZE]);
            Ok(page_id)
        }
    }

    #[test]
    fn three_frames_evict_and_persist_twenty_pages_after_shutdown() {
        let path = std::env::temp_dir().join(format!(
            "lidb-buffer-pool-{}-{}.db",
            std::process::id(),
            TEST_FILE_NUMBER.fetch_add(1, Ordering::Relaxed)
        ));
        let disk = FileDiskManager::create(&path).unwrap();
        let mut pool = BufferPool::new(disk, 3);

        for expected_id in 0..20_u64 {
            let page_id = pool.allocate_page().unwrap();
            assert_eq!(page_id, PageId(expected_id));
            let frame = pool.fetch_page(page_id).unwrap();
            frame.data[0..8].copy_from_slice(&expected_id.to_le_bytes());
            pool.unpin_page(page_id, true).unwrap();
        }

        // Reading and writing in reverse repeatedly forces misses and LRU
        // evictions because twenty pages cannot fit in three frames.
        for _ in 0..2 {
            for expected_id in (0..20_u64).rev() {
                let page_id = PageId(expected_id);
                let frame = pool.fetch_page(page_id).unwrap();
                assert_eq!(
                    u64::from_le_bytes(frame.data[0..8].try_into().unwrap()),
                    expected_id
                );
                frame.data[8] = frame.data[8].wrapping_add(1);
                pool.unpin_page(page_id, true).unwrap();
            }
        }
        drop(pool.shutdown().unwrap());

        let disk = FileDiskManager::open(&path).unwrap();
        let mut reopened = BufferPool::new(disk, 3);
        for expected_id in 0..20_u64 {
            let page_id = PageId(expected_id);
            let frame = reopened.fetch_page(page_id).unwrap();
            assert_eq!(
                u64::from_le_bytes(frame.data[0..8].try_into().unwrap()),
                expected_id
            );
            assert_eq!(frame.data[8], 2);
            reopened.unpin_page(page_id, false).unwrap();
        }
        drop(reopened.shutdown().unwrap());
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn pinned_pages_are_never_victims() {
        let disk = MemoryDisk {
            pages: vec![[0; PAGE_SIZE]; 2],
        };
        let mut pool = BufferPool::new(disk, 1);
        pool.fetch_page(PageId(0)).unwrap();

        assert!(matches!(
            pool.fetch_page(PageId(1)),
            Err(DbError::AllFramesPinned)
        ));
        pool.unpin_page(PageId(0), false).unwrap();
        pool.fetch_page(PageId(1)).unwrap();
    }
}