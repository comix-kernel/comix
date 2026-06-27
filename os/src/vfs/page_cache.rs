//! Clean file page cache shared by VFS-backed filesystems.
//!
//! This cache only stores clean file data. Filesystems remain responsible for
//! all writes and must invalidate affected clean pages after successful
//! mutations.

use alloc::collections::BTreeMap;
use alloc::sync::Arc;
use alloc::vec::Vec;

use crate::arch::pa_to_va;
use crate::mm::address::PageNum;
use crate::mm::frame_allocator::{FrameTracker, alloc_frame};
use crate::sync::SpinLock;
use crate::vfs::FsError;

/// Size of one cached file page.
pub const PAGE_CACHE_PAGE_SIZE: usize = 4096;

/// Default maximum number of clean pages retained by a page cache.
pub const DEFAULT_PAGE_CACHE_MAX_PAGES: usize = 512;

/// Stable identity for one cacheable file object.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct PageCacheObjectId {
    /// Filesystem instance id. This must differ between mounts/devices.
    pub fs_id: u64,
    /// Inode number inside the filesystem instance.
    pub inode_no: u64,
}

impl PageCacheObjectId {
    /// Creates a file object identity from a filesystem id and inode number.
    pub const fn new(fs_id: u64, inode_no: u64) -> Self {
        Self { fs_id, inode_no }
    }
}

/// Key for one cached file page.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct PageCacheKey {
    /// File object this page belongs to.
    pub object: PageCacheObjectId,
    /// Zero-based page index within the file.
    pub page_index: usize,
}

impl PageCacheKey {
    /// Creates a cache key for one file page.
    pub const fn new(object: PageCacheObjectId, page_index: usize) -> Self {
        Self { object, page_index }
    }
}

/// Backing storage for a clean cached file page.
#[derive(Clone, Debug)]
enum CachedPageStorage {
    Bytes(Vec<u8>),
    Frame(Arc<FrameTracker>, usize),
}

impl CachedPageStorage {
    fn from_bytes(mut data: Vec<u8>) -> Self {
        data.truncate(PAGE_CACHE_PAGE_SIZE);
        Self::Bytes(data)
    }

    fn as_slice(&self) -> &[u8] {
        match self {
            Self::Bytes(data) => data,
            Self::Frame(frame, len) => {
                let va = pa_to_va(frame.ppn().start_addr());
                unsafe { core::slice::from_raw_parts(va.as_usize() as *const u8, *len) }
            }
        }
    }

    fn as_mut_page_slice(&mut self) -> &mut [u8] {
        match self {
            Self::Bytes(data) => data.as_mut_slice(),
            Self::Frame(frame, _) => {
                let va = pa_to_va(frame.ppn().start_addr());
                unsafe {
                    core::slice::from_raw_parts_mut(va.as_usize() as *mut u8, PAGE_CACHE_PAGE_SIZE)
                }
            }
        }
    }

    fn is_frame_backed(&self) -> bool {
        matches!(self, Self::Frame(_, _))
    }
}

/// A clean cached file page.
#[derive(Clone, Debug)]
pub struct CachedPage {
    storage: CachedPageStorage,
}

impl CachedPage {
    /// Creates a clean cached page, truncating data to one page.
    pub fn new(data: Vec<u8>) -> Self {
        Self {
            storage: CachedPageStorage::from_bytes(data),
        }
    }

    /// Allocates a frame-backed clean cached page initialized from `data`.
    pub fn new_frame_backed(data: &[u8]) -> Result<Self, FsError> {
        let mut page = Self {
            storage: CachedPageStorage::Frame(
                Arc::new(alloc_frame().ok_or(FsError::NoMemory)?),
                data.len().min(PAGE_CACHE_PAGE_SIZE),
            ),
        };
        page.write_prefix(data);
        Ok(page)
    }

    /// Returns the bytes stored in this clean page.
    pub fn data(&self) -> &[u8] {
        self.storage.as_slice()
    }

    /// Copies bytes from this page into `buf`, starting at `page_offset`.
    pub fn copy_out(&self, page_offset: usize, buf: &mut [u8]) -> usize {
        let data = self.data();
        if page_offset >= data.len() {
            return 0;
        }

        let n = (data.len() - page_offset).min(buf.len());
        buf[..n].copy_from_slice(&data[page_offset..page_offset + n]);
        n
    }

    fn write_prefix(&mut self, data: &[u8]) {
        let len = data.len().min(PAGE_CACHE_PAGE_SIZE);
        let dst = self.storage.as_mut_page_slice();
        dst[..len].copy_from_slice(&data[..len]);
    }

    fn full_page_mut(&mut self) -> &mut [u8] {
        self.storage.as_mut_page_slice()
    }

    fn set_len(&mut self, len: usize) {
        let len = len.min(PAGE_CACHE_PAGE_SIZE);
        match &mut self.storage {
            CachedPageStorage::Bytes(data) => data.truncate(len),
            CachedPageStorage::Frame(_, page_len) => *page_len = len,
        }
    }

    /// Returns whether this page is backed by a physical frame.
    pub fn is_frame_backed(&self) -> bool {
        self.storage.is_frame_backed()
    }
}

/// Page cache counters.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct PageCacheStats {
    /// Number of successful page lookups.
    pub hits: usize,
    /// Number of failed page lookups.
    pub misses: usize,
    /// Number of clean page insertions.
    pub inserts: usize,
    /// Number of pages evicted by capacity pressure.
    pub evicts: usize,
    /// Number of pages removed by explicit invalidation.
    pub invalidates: usize,
    /// Number of fill closures that returned an error.
    pub fill_errors: usize,
    /// Current number of cached pages.
    pub resident_pages: usize,
    /// Current number of cached pages backed by physical frames.
    pub frame_pages: usize,
}

struct CacheEntry {
    page: CachedPage,
    age: u64,
}

struct PageCacheInner {
    pages: BTreeMap<PageCacheKey, CacheEntry>,
    clock: u64,
    stats: PageCacheStats,
}

impl PageCacheInner {
    const fn new() -> Self {
        Self {
            pages: BTreeMap::new(),
            clock: 0,
            stats: PageCacheStats {
                hits: 0,
                misses: 0,
                inserts: 0,
                evicts: 0,
                invalidates: 0,
                fill_errors: 0,
                resident_pages: 0,
                frame_pages: 0,
            },
        }
    }

    fn tick(&mut self) -> u64 {
        self.clock = self.clock.wrapping_add(1);
        self.clock
    }

    fn evict_until_space(&mut self, max_pages: usize) {
        while self.pages.len() >= max_pages {
            let Some(oldest_key) = self
                .pages
                .iter()
                .min_by_key(|(_, entry)| entry.age)
                .map(|(key, _)| *key)
            else {
                break;
            };
            self.pages.remove(&oldest_key);
            self.stats.evicts += 1;
        }
    }

    fn stats_snapshot(&self) -> PageCacheStats {
        let mut stats = self.stats;
        stats.resident_pages = self.pages.len();
        stats.frame_pages = self
            .pages
            .values()
            .filter(|entry| entry.page.is_frame_backed())
            .count();
        stats
    }
}

/// Capacity-limited clean file page cache.
pub struct PageCache {
    max_pages: usize,
    inner: SpinLock<PageCacheInner>,
}

impl PageCache {
    /// Creates a clean page cache with a fixed page capacity.
    pub const fn with_capacity(max_pages: usize) -> Self {
        Self {
            max_pages,
            inner: SpinLock::new(PageCacheInner::new()),
        }
    }

    /// Creates a clean page cache using the default page capacity.
    pub const fn new() -> Self {
        Self::with_capacity(DEFAULT_PAGE_CACHE_MAX_PAGES)
    }

    /// Returns a cloned clean page for `key` if it is cached.
    pub fn lookup(&self, key: PageCacheKey) -> Option<CachedPage> {
        let mut inner = self.inner.lock();
        let age = inner.tick();
        if inner.pages.contains_key(&key) {
            let page = {
                let entry = inner.pages.get_mut(&key).unwrap();
                entry.age = age;
                entry.page.clone()
            };
            inner.stats.hits += 1;
            Some(page)
        } else {
            inner.stats.misses += 1;
            None
        }
    }

    /// Copies a cached range from one page into `buf`.
    ///
    /// Returns `None` when the page containing `offset` is not cached. Returns
    /// `Some(0)` when `offset` lies past the cached page data.
    pub fn read_hit(
        &self,
        object: PageCacheObjectId,
        offset: usize,
        buf: &mut [u8],
    ) -> Option<usize> {
        let page_index = offset / PAGE_CACHE_PAGE_SIZE;
        let page_offset = offset % PAGE_CACHE_PAGE_SIZE;
        let page = self.lookup(PageCacheKey::new(object, page_index))?;

        if page_offset >= page.data().len() {
            return Some(0);
        }

        let n = (page.data().len() - page_offset).min(buf.len());
        buf[..n].copy_from_slice(&page.data()[page_offset..page_offset + n]);
        Some(n)
    }

    /// Inserts a clean page for `object` at `page_index`.
    pub fn insert_clean(&self, object: PageCacheObjectId, page_index: usize, data: Vec<u8>) {
        if self.max_pages == 0 || data.is_empty() {
            return;
        }

        let key = PageCacheKey::new(object, page_index);
        let mut inner = self.inner.lock();
        let age = inner.tick();

        if !inner.pages.contains_key(&key) {
            inner.evict_until_space(self.max_pages);
        }

        inner.pages.insert(key, CacheEntry {
            page: CachedPage::new(data),
            age,
        });
        inner.stats.inserts += 1;
    }

    /// Returns an existing clean page or fills and inserts a new frame-backed page.
    pub fn get_or_insert_clean_page<F>(
        &self,
        object: PageCacheObjectId,
        page_index: usize,
        fill_fn: F,
    ) -> Result<CachedPage, FsError>
    where
        F: FnOnce(&mut [u8]) -> Result<usize, FsError>,
    {
        let key = PageCacheKey::new(object, page_index);

        if let Some(page) = self.lookup(key) {
            return Ok(page);
        }

        if self.max_pages == 0 {
            let mut data = alloc::vec![0u8; PAGE_CACHE_PAGE_SIZE];
            let len = match fill_fn(&mut data) {
                Ok(len) => len.min(PAGE_CACHE_PAGE_SIZE),
                Err(err) => {
                    self.inner.lock().stats.fill_errors += 1;
                    return Err(err);
                }
            };
            data.truncate(len);
            return Ok(CachedPage::new(data));
        }

        let mut page = CachedPage::new_frame_backed(&[])?;
        let len = {
            let buffer = page.full_page_mut();
            match fill_fn(buffer) {
                Ok(len) => len.min(PAGE_CACHE_PAGE_SIZE),
                Err(err) => {
                    self.inner.lock().stats.fill_errors += 1;
                    return Err(err);
                }
            }
        };
        page.set_len(len);

        if len == 0 {
            return Ok(page);
        }

        let mut inner = self.inner.lock();
        let age = inner.tick();
        if !inner.pages.contains_key(&key) {
            inner.evict_until_space(self.max_pages);
        }
        inner.pages.insert(key, CacheEntry {
            page: page.clone(),
            age,
        });
        inner.stats.inserts += 1;
        Ok(page)
    }

    /// Invalidates cached pages intersecting byte range `[offset, offset + len)`.
    pub fn invalidate_range(&self, object: PageCacheObjectId, offset: usize, len: usize) {
        if len == 0 {
            return;
        }

        let start = offset / PAGE_CACHE_PAGE_SIZE;
        let last = offset.saturating_add(len - 1) / PAGE_CACHE_PAGE_SIZE;
        let end = last.saturating_add(1);

        let mut inner = self.inner.lock();
        let before = inner.pages.len();
        inner.pages.retain(|key, _| {
            key.object != object || key.page_index < start || key.page_index >= end
        });
        inner.stats.invalidates += before - inner.pages.len();
    }

    /// Invalidates every cached page for one file object.
    pub fn invalidate_inode(&self, object: PageCacheObjectId) {
        let mut inner = self.inner.lock();
        let before = inner.pages.len();
        inner.pages.retain(|key, _| key.object != object);
        inner.stats.invalidates += before - inner.pages.len();
    }

    /// Invalidates every cached page for one filesystem instance.
    pub fn invalidate_fs(&self, fs_id: u64) {
        let mut inner = self.inner.lock();
        let before = inner.pages.len();
        inner.pages.retain(|key, _| key.object.fs_id != fs_id);
        inner.stats.invalidates += before - inner.pages.len();
    }

    /// Returns a snapshot of page cache counters.
    pub fn stats(&self) -> PageCacheStats {
        self.inner.lock().stats_snapshot()
    }
}

impl Default for PageCache {
    fn default() -> Self {
        Self::new()
    }
}
