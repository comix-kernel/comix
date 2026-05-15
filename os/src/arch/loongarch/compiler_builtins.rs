//! Minimal compiler builtin shims required by the LoongArch target.

use core::ptr;

/// C ABI `memcpy` replacement for LoongArch bare-metal builds.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn memcpy(dst: *mut u8, src: *const u8, n: usize) -> *mut u8 {
    for i in 0..n {
        unsafe {
            ptr::write(dst.add(i), ptr::read(src.add(i)));
        }
    }
    dst
}

/// C ABI `memmove` replacement for LoongArch bare-metal builds.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn memmove(dst: *mut u8, src: *const u8, n: usize) -> *mut u8 {
    if dst as usize == src as usize || n == 0 {
        return dst;
    }
    if (dst as usize) < (src as usize) || (dst as usize) >= (src as usize + n) {
        for i in 0..n {
            unsafe {
                ptr::write(dst.add(i), ptr::read(src.add(i)));
            }
        }
    } else {
        for i in (0..n).rev() {
            unsafe {
                ptr::write(dst.add(i), ptr::read(src.add(i)));
            }
        }
    }
    dst
}

/// C ABI `memset` replacement for LoongArch bare-metal builds.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn memset(dst: *mut u8, c: i32, n: usize) -> *mut u8 {
    let val = c as u8;
    for i in 0..n {
        unsafe {
            ptr::write(dst.add(i), val);
        }
    }
    dst
}
