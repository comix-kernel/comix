//! 编译器内建函数替代实现 (LoongArch)
//!
//! LoongArch64 目标的 compiler_builtins crate 不完全支持，
//! 因此提供纯 Rust 的 memcpy/memmove/memset 实现。
#[cfg(target_arch = "loongarch64")]
use core::ptr;

#[cfg(target_arch = "loongarch64")]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn memcpy(dst: *mut u8, src: *const u8, n: usize) -> *mut u8 {
    for i in 0..n {
        ptr::write(dst.add(i), ptr::read(src.add(i)));
    }
    dst
}

#[cfg(target_arch = "loongarch64")]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn memmove(dst: *mut u8, src: *const u8, n: usize) -> *mut u8 {
    if dst as usize == src as usize || n == 0 {
        return dst;
    }
    if (dst as usize) < (src as usize) || (dst as usize) >= (src as usize + n) {
        for i in 0..n {
            ptr::write(dst.add(i), ptr::read(src.add(i)));
        }
    } else {
        for i in (0..n).rev() {
            ptr::write(dst.add(i), ptr::read(src.add(i)));
        }
    }
    dst
}

#[cfg(target_arch = "loongarch64")]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn memset(dst: *mut u8, c: i32, n: usize) -> *mut u8 {
    let val = c as u8;
    for i in 0..n {
        ptr::write(dst.add(i), val);
    }
    dst
}
