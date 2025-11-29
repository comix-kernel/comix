use core::ptr::{read_volatile, write_volatile};

use alloc::{
    string::{String, ToString},
    vec::Vec,
};

use crate::config::MAX_ARGV;

/// 向指定地址写入内容
/// # 参数：
/// * `addr` - 目标地址
/// * `content` - 要写入的内容
#[inline(always)]
pub fn write<T>(addr: usize, content: T) {
    let cell = (addr) as *mut T;
    unsafe {
        write_volatile(cell, content);
    }
}

/// 从指定地址读取内容
/// # 参数：
/// * `addr` - 目标地址
/// # 返回值：
/// 读取到的内容
#[inline(always)]
pub fn read<T>(addr: usize) -> T {
    let cell = (addr) as *const T;
    unsafe { read_volatile(cell) }
}

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
        match unsafe { crate::util::copy_cstr_to_string(p) } {
            Ok(s) => out.push(s),
            Err(_) => return Err(()),
        }
    }
    Ok(out)
}

/// 计算字符串的长度（不包括 NULL 终止符）
/// # 参数：
/// * `s` - 字符串指针
/// # 返回值：
/// 字符串长度
pub unsafe fn cstr_len(s: *const u8) -> usize {
    let mut len = 0;
    let mut p = s;
    while unsafe { core::ptr::read(p) } != 0 {
        len += 1;
        p = unsafe { p.add(1) };
    }
    len
}

/// 比较两个 C 字符串是否相等
/// # 参数：
/// * `s1` - 第一个字符串指针
/// * `s2` - 第二个字符串指针
/// # 返回值：
/// 如果相等返回 true，否则返回 false
pub unsafe fn cstr_equal(s1: *const u8, s2: *const u8) -> bool {
    let mut p1 = s1;
    let mut p2 = s2;
    loop {
        let b1 = unsafe { core::ptr::read(p1) };
        let b2 = unsafe { core::ptr::read(p2) };
        if b1 != b2 {
            return false;
        }
        if b1 == 0 {
            return true;
        }
        p1 = unsafe { p1.add(1) };
        p2 = unsafe { p2.add(1) };
    }
}

/// 从源指针拷贝 C 字符串到目标缓冲区
/// # 参数：
/// * `src` - 源字符串指针
/// * `dest` - 目标缓冲区切片
/// * `len` - 最大拷贝长度
pub fn cstr_copy(src: *const u8, dest: &mut [u8], len: usize) {
    for i in 0..len {
        let b = unsafe { core::ptr::read(src.add(i)) };
        dest[i] = b;
        if b == 0 {
            break;
        }
    }
}
