//! LoongArch64 memory helpers.
//!
//! Provide simple byte-wise implementations to avoid unsupported instructions
//! in QEMU/LoongArch for compiler_builtins memcpy/memmove/memset.

use core::ptr;

#[unsafe(no_mangle)]
pub unsafe extern "C" fn memcpy(dst: *mut u8, src: *const u8, n: usize) -> *mut u8 {
    for i in 0..n {
        ptr::write(dst.add(i), ptr::read(src.add(i)));
    }
    dst
}

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

#[unsafe(no_mangle)]
pub unsafe extern "C" fn memset(dst: *mut u8, c: i32, n: usize) -> *mut u8 {
    let val = c as u8;
    for i in 0..n {
        ptr::write(dst.add(i), val);
    }
    dst
}
