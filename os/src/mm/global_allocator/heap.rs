#![allow(dead_code)]
// 内核堆（Kernel Heap）的公共接口，提供了 C 风格的 kmalloc 家族函数。

// 引入在 kalloc.rs 中实现的全局分配器
// #[global_allocator]
// static ALLOCATOR: ...;
//
// 假设全局分配器 ALLOCATOR 已经被正确设置和初始化。

// -------------------------------------------------------------------
// 用于导出到外部的 C 风格的 kmalloc 家族接口 (Wrapper Functions)
// -------------------------------------------------------------------

/// kmalloc: 内核动态内存分配函数。
///
/// 功能: 分配至少 `size` 字节的内存块。返回的内存是**未初始化**的。
///
/// # Arguments
/// * `size`: 需要分配的字节数。
///
/// # Safety
/// 这是一个不安全函数，因为分配失败可能返回空指针，且使用者必须负责调用 kfree 释放。
///
/// # Returns
/// 分配内存块的指针 (*mut u8)；失败则返回 ptr::null_mut()。
#[inline]
pub unsafe fn kmalloc(_size: usize) -> *mut u8 {
    unimplemented!()
}

/// kcalloc: 内核动态内存分配并清零函数。
///
/// 功能: 分配 `count * size` 字节的内存块，并将分配的内存**清零**。
///
/// # Arguments
/// * `count`: 元素数量。
/// * `size`: 每个元素的字节数。
///
/// # Safety
/// 必须负责释放。如果乘法溢出或分配失败，返回空指针。
///
/// # Returns
/// 分配内存块的指针 (*mut u8)；失败则返回 ptr::null_mut()。
#[inline]
pub unsafe fn kcalloc(_count: usize, _size: usize) -> *mut u8 {
    unimplemented!()
}

/// kfree: 释放之前由 kmalloc/kcalloc/krealloc 分配的内存块。
///
/// # Arguments
/// * `ptr`: 待释放的内存块指针。
/// * `size`: 释放的内存块大小。
///
/// # Safety
/// * `ptr` 必须是由本分配器分配的。
/// * `ptr` 必须是非空且尚未被释放。
/// * `size` 必须与分配时的 `size` 相同。
#[inline]
pub unsafe fn kfree(_ptr: *mut u8, _size: usize) {
    unimplemented!()
}

/// krealloc: 重新调整内存块的大小。
///
/// 功能: 调整 `old_ptr` 指向的内存块大小为 `new_size`。
///
/// # Arguments
/// * `old_ptr`: 原始内存块的指针。
/// * `old_size`: 原始内存块的大小。
/// * `new_size`: 新需要的大小。
///
/// # Safety
/// * `old_ptr` 必须是本分配器分配的，且未释放。
/// * `old_size` 必须与分配时的大小相同。
///
/// # Returns
/// 新的内存块指针 (*mut u8)；失败则返回 ptr::null_mut()。
#[inline]
pub unsafe fn krealloc(_old_ptr: *mut u8, _old_size: usize, _new_size: usize) -> *mut u8 {
    unimplemented!()
}
