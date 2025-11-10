//! 工具函数模块
use alloc::{
    string::{String, ToString},
    vec::Vec,
};

/// 从以 NULL 结尾的 C 字符串指针拷贝并返回一个 owned String
/// WARNING: 这个函数直接读取指针，调用者必须保证指针在内核可读（若为用户指针请改为使用 MemorySpace 的读用户内存接口）
pub unsafe fn copy_cstr_to_string(ptr: *const u8) -> Result<String, ()> {
    const MAX_PATH_LEN: usize = 4096;
    let mut buf: Vec<u8> = Vec::new();
    let mut p = ptr;
    for _ in 0..MAX_PATH_LEN {
        // 直接读取内存字节（不安全）
        let b = unsafe { core::ptr::read(p) };
        if b == 0 {
            return core::str::from_utf8(&buf)
                .map(|s| s.to_string())
                .map_err(|_| ());
        }
        buf.push(b);
        p = unsafe { p.add(1) };
    }
    Err(())
}
