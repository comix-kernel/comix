use crate::kassert;
use crate::test_case;
use crate::vfs::FsError;
use crate::vfs::page_cache::{
    CachedPage, PAGE_CACHE_PAGE_SIZE, PageCache, PageCacheKey, PageCacheObjectId,
};
use alloc::vec;
use core::sync::atomic::{AtomicUsize, Ordering};

fn object(fs_id: u64, inode_no: u64) -> PageCacheObjectId {
    PageCacheObjectId::new(fs_id, inode_no)
}

test_case!(test_page_cache_lookup_and_read_hit, {
    let cache = PageCache::with_capacity(4);
    let obj = object(1, 2);

    cache.insert_clean(obj, 0, b"hello".to_vec());

    let page = cache.lookup(PageCacheKey::new(obj, 0)).unwrap();
    kassert!(page.data() == b"hello");

    let mut buf = [0u8; 3];
    let n = cache.read_hit(obj, 1, &mut buf).unwrap();
    kassert!(n == 3);
    kassert!(&buf == b"ell");

    let stats = cache.stats();
    kassert!(stats.hits == 2);
    kassert!(stats.misses == 0);
    kassert!(stats.inserts == 1);
});

test_case!(test_page_cache_miss_counter, {
    let cache = PageCache::with_capacity(4);
    let obj = object(1, 2);
    let mut buf = [0u8; 4];

    kassert!(cache.read_hit(obj, 0, &mut buf).is_none());

    let stats = cache.stats();
    kassert!(stats.hits == 0);
    kassert!(stats.misses == 1);
});

test_case!(test_page_cache_cross_page_read, {
    let cache = PageCache::with_capacity(4);
    let obj = object(1, 2);

    cache.insert_clean(obj, 0, vec![b'a'; PAGE_CACHE_PAGE_SIZE]);
    cache.insert_clean(obj, 1, b"bcdef".to_vec());

    let mut first = [0u8; 4];
    let n = cache
        .read_hit(obj, PAGE_CACHE_PAGE_SIZE - first.len(), &mut first)
        .unwrap();
    kassert!(n == first.len());
    kassert!(first == [b'a'; 4]);

    let mut second = [0u8; 5];
    let n = cache
        .read_hit(obj, PAGE_CACHE_PAGE_SIZE, &mut second)
        .unwrap();
    kassert!(n == second.len());
    kassert!(&second == b"bcdef");
});

test_case!(test_page_cache_lru_eviction, {
    let cache = PageCache::with_capacity(2);
    let obj = object(1, 2);

    cache.insert_clean(obj, 0, b"zero".to_vec());
    cache.insert_clean(obj, 1, b"one".to_vec());
    kassert!(cache.lookup(PageCacheKey::new(obj, 0)).is_some());
    cache.insert_clean(obj, 2, b"two".to_vec());

    kassert!(cache.lookup(PageCacheKey::new(obj, 0)).is_some());
    kassert!(cache.lookup(PageCacheKey::new(obj, 1)).is_none());
    kassert!(cache.lookup(PageCacheKey::new(obj, 2)).is_some());

    let stats = cache.stats();
    kassert!(stats.evicts == 1);
});

test_case!(test_page_cache_range_inode_and_fs_invalidation, {
    let cache = PageCache::with_capacity(8);
    let obj1 = object(1, 10);
    let obj2 = object(1, 11);
    let obj3 = object(2, 10);

    cache.insert_clean(obj1, 0, b"a".to_vec());
    cache.insert_clean(obj1, 1, b"b".to_vec());
    cache.insert_clean(obj2, 0, b"c".to_vec());
    cache.insert_clean(obj3, 0, b"d".to_vec());

    cache.invalidate_range(obj1, PAGE_CACHE_PAGE_SIZE - 1, 2);
    kassert!(cache.lookup(PageCacheKey::new(obj1, 0)).is_none());
    kassert!(cache.lookup(PageCacheKey::new(obj1, 1)).is_none());
    kassert!(cache.lookup(PageCacheKey::new(obj2, 0)).is_some());

    cache.invalidate_inode(obj2);
    kassert!(cache.lookup(PageCacheKey::new(obj2, 0)).is_none());
    kassert!(cache.lookup(PageCacheKey::new(obj3, 0)).is_some());

    cache.invalidate_fs(2);
    kassert!(cache.lookup(PageCacheKey::new(obj3, 0)).is_none());

    let stats = cache.stats();
    kassert!(stats.invalidates == 4);
});

test_case!(test_page_cache_object_id_includes_fs_id, {
    let cache = PageCache::with_capacity(4);
    let left = object(1, 7);
    let right = object(2, 7);

    cache.insert_clean(left, 0, b"left".to_vec());
    cache.insert_clean(right, 0, b"right".to_vec());

    let mut buf = [0u8; 5];
    let n = cache.read_hit(right, 0, &mut buf).unwrap();
    kassert!(n == 5);
    kassert!(&buf == b"right");
});

test_case!(test_frame_backed_cached_page_copy_out, {
    let page = CachedPage::new_frame_backed(b"frame-data").unwrap();

    kassert!(page.data() == b"frame-data");

    let mut buf = [0u8; 5];
    let n = page.copy_out(6, &mut buf);
    kassert!(n == 4);
    kassert!(&buf[..n] == b"data");
});

test_case!(test_frame_backed_cached_page_truncates_to_page, {
    let oversized = vec![0xAB; PAGE_CACHE_PAGE_SIZE + 17];
    let page = CachedPage::new_frame_backed(&oversized).unwrap();

    kassert!(page.data().len() == PAGE_CACHE_PAGE_SIZE);
    kassert!(page.data()[0] == 0xAB);
    kassert!(page.data()[PAGE_CACHE_PAGE_SIZE - 1] == 0xAB);

    let mut buf = [0u8; 8];
    let n = page.copy_out(PAGE_CACHE_PAGE_SIZE - 4, &mut buf);
    kassert!(n == 4);
    kassert!(&buf[..n] == &[0xAB; 4]);
});

test_case!(test_get_or_insert_clean_page_fills_miss_once, {
    let cache = PageCache::with_capacity(4);
    let obj = object(1, 42);
    let fills = AtomicUsize::new(0);

    let page = cache
        .get_or_insert_clean_page(obj, 0, |buf| {
            fills.fetch_add(1, Ordering::Relaxed);
            buf[..5].copy_from_slice(b"first");
            Ok(5)
        })
        .unwrap();
    kassert!(page.data() == b"first");

    let cached = cache
        .get_or_insert_clean_page(obj, 0, |_| {
            fills.fetch_add(1, Ordering::Relaxed);
            Ok(0)
        })
        .unwrap();
    kassert!(cached.data() == b"first");
    kassert!(fills.load(Ordering::Relaxed) == 1);

    let stats = cache.stats();
    kassert!(stats.inserts == 1);
});

test_case!(test_get_or_insert_clean_page_fill_error_does_not_insert, {
    let cache = PageCache::with_capacity(4);
    let obj = object(1, 43);
    let mut buf = [0u8; 4];

    let result = cache.get_or_insert_clean_page(obj, 0, |_| Err(FsError::IoError));
    kassert!(matches!(result, Err(FsError::IoError)));
    kassert!(cache.read_hit(obj, 0, &mut buf).is_none());

    let stats = cache.stats();
    kassert!(stats.inserts == 0);
});
