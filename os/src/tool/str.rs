use alloc::{
    string::{String, ToString},
    vec::Vec,
};

use crate::config::MAX_ARGV;

/// 从以 NULL 结尾的 C 字符串指针拷贝并返回一个 owned String
/// WARNING: 这个函数直接读取指针，调用者必须保证指针在内核可读
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

/// 把 NULL 终止的指针数组拷贝为 `Vec<String>`
/// WARNING: 这个函数直接读取指针，调用者必须保证指针在内核可读
pub unsafe fn ptr_array_to_vec_strings(ptrs: *const *const u8) -> Result<Vec<String>, ()> {
    let mut out: Vec<String> = Vec::new();
    if ptrs.is_null() {
        return Ok(out);
    }
    for i in 0..MAX_ARGV {
        let p = unsafe { *ptrs.add(i) };
        if p.is_null() {
            break;
        }
        match unsafe { crate::tool::copy_cstr_to_string(p) } {
            Ok(s) => out.push(s),
            Err(_) => return Err(()),
        }
    }
    Ok(out)
}
