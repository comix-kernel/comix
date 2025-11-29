//! 全局分配器模块
//!
//! 本模块使用 **talc** 分配器提供动态堆内存分配功能。
//!
//! # 模块组成
//!
//! - 基于 **talc::Talck** 的全局堆分配器。
//! - 由链接器符号定义的堆内存区域。
//! - 用于设置堆的初始化函数。

use crate::{earlyprintln, println, sync::RawSpinLockWithoutGuard};
use talc::{Span, Talc, Talck};

/// 全局堆分配器实例
///
/// 使用 talc 的基于锁的分配器 (**Talck**) 和我们自定义的 **`RawSpinLockWithoutGuard`**。
/// 此锁实现了 `lock_api::RawMutex` 并提供了中断保护，
/// 以防止当中断处理程序尝试分配内存时发生死锁。
///
/// 初始化时使用一个空范围 (**Span::empty()**)；实际内存将在 `init_heap()` 中声明。
#[global_allocator]
static ALLOCATOR: Talck<RawSpinLockWithoutGuard, talc::ClaimOnOom> =
    Talc::new(unsafe { talc::ClaimOnOom::new(Span::empty()) }).lock();

/// 使用链接器脚本中定义的堆内存区域初始化堆分配器
///
/// 此函数必须在启动过程的早期调用，即在 BSS 清零之后，
/// 且在任何堆分配尝试之前。
///
/// # 安全性
///
/// - 在启动过程中必须且只能调用一次。
/// - 必须在进行任何堆分配之前调用。
/// - 由链接器符号 (`sheap`, `eheap`) 定义的堆区域必须有效。
pub fn init_heap() {
    unsafe extern "C" {
        fn sheap();
        fn eheap();
    }

    let heap_start = sheap as usize;
    let heap_end = eheap as usize;
    let heap_size = heap_end - heap_start;

    earlyprintln!(
        "Initializing heap: start={:#x}, end={:#x}, size={:#x} ({} MB)",
        heap_start,
        heap_end,
        heap_size,
        heap_size / 1024 / 1024
    );

    unsafe {
        ALLOCATOR
            .lock()
            .claim(Span::new(heap_start as *mut u8, heap_end as *mut u8))
            .expect("Failed to initialize heap allocator");
    }

    earlyprintln!("Heap allocator initialized successfully");
}
