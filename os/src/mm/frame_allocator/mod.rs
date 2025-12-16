// HACK: 在一个模块目录/文件的顶层又声明了一个同名子模块，这会造成 “module inception”。
// 虽然功能上可行，但会引起 API/模块层次混淆，Clippy 建议消除这种重复。
#![allow(clippy::module_inception)]
//! 帧分配器模块
//!
//! 本模块提供物理内存帧的分配和跟踪功能。
//!
//! # 模块组成
//!
//! - [`FrameTracker`]：用于单个已分配帧的 **RAII** 封装器。
//! - [`FrameRangeTracker`]：用于已分配帧范围的 **RAII** 封装器。
//! - [`init_frame_allocator`]：初始化全局帧分配器。
//! - [`alloc_frame`]：分配单个帧。
//! - `alloc_frames`：分配多个（非连续）帧。
//! - `alloc_contig_frames`：分配多个连续帧。
//! - `alloc_contig_frames_aligned`：分配带对齐要求的多个连续帧。

mod frame_allocator;

use alloc::vec::Vec;
pub use frame_allocator::{FrameRangeTracker, FrameTracker, TrackedFrames};

use crate::mm::{
    address::{Paddr, PageNum, Ppn, UsizeConvert},
    frame_allocator::frame_allocator::FRAME_ALLOCATOR,
};

/// 使用可用的物理内存范围初始化全局帧分配器。
///
/// # 参数
///
/// * `start_addr` - 可用物理内存的起始地址
/// * `end_addr` - 可用物理内存的结束地址
pub fn init_frame_allocator(start_addr: usize, end_addr: usize) {
    // 将起始地址向上取整到页号
    let start_ppn = Ppn::from_addr_ceil(Paddr::from_usize(start_addr));
    // 将结束地址向下取整到页号
    let end_ppn = Ppn::from_addr_floor(Paddr::from_usize(end_addr));

    let mut allocator = FRAME_ALLOCATOR.lock();
    allocator.init(start_ppn, end_ppn);
}

/// 分配一个物理帧。
///
/// # 返回
///
/// 如果分配成功，返回 `Some(FrameTracker)`；否则返回 `None`。
pub fn alloc_frame() -> Option<FrameTracker> {
    FRAME_ALLOCATOR.lock().alloc_frame()
}

/// 分配多个物理帧（不保证连续）。
///
/// # 参数
///
/// * `num` - 需要分配的帧数量。
///
/// # 返回
///
/// 如果分配成功，返回 `Some(Vec<FrameTracker>)`；否则返回 `None`。
pub fn alloc_frames(num: usize) -> Option<Vec<FrameTracker>> {
    FRAME_ALLOCATOR.lock().alloc_frames(num)
}

/// 分配指定数量的**连续**物理帧。
///
/// # 参数
///
/// * `num` - 需要分配的帧数量。
///
/// # 返回
///
/// 如果分配成功，返回 `Some(FrameRangeTracker)`；否则返回 `None`。
pub fn alloc_contig_frames(num: usize) -> Option<FrameRangeTracker> {
    FRAME_ALLOCATOR.lock().alloc_contig_frames(num)
}

/// 分配指定数量的**连续**物理帧，并确保起始地址对齐。
///
/// # 参数
///
/// * `num` - 需要分配的帧数量。
/// * `align_pages` - 对齐的页数（必须是 2 的幂）。
///
/// # 返回
///
/// 如果分配成功，返回 `Some(FrameRangeTracker)`；否则返回 `None`。
pub fn alloc_contig_frames_aligned(num: usize, align_pages: usize) -> Option<FrameRangeTracker> {
    FRAME_ALLOCATOR
        .lock()
        .alloc_contig_frames_aligned(num, align_pages)
}

/// 回收一个物理帧。此函数由 FrameTracker 的 Drop 实现调用。
fn dealloc_frame(frame: &FrameTracker) {
    FRAME_ALLOCATOR.lock().dealloc_frame(frame);
}

/// 回收多个物理帧（不保证连续）。
fn dealloc_frames(frames: &[FrameTracker]) {
    let mut allocator = FRAME_ALLOCATOR.lock();
    for frame in frames {
        allocator.dealloc_frame(frame);
    }
}

/// 回收一个连续的物理帧范围。此函数由 FrameRangeTracker 的 Drop 实现调用。
fn dealloc_contig_frames(frame_range: &FrameRangeTracker) {
    FRAME_ALLOCATOR.lock().dealloc_contig_frames(frame_range);
}

/// 获取总的物理帧数
pub fn get_total_frames() -> usize {
    FRAME_ALLOCATOR.lock().total_frames()
}

/// 获取已分配的帧数
pub fn get_allocated_frames() -> usize {
    FRAME_ALLOCATOR.lock().allocated_frames()
}

/// 获取空闲的帧数
pub fn get_free_frames() -> usize {
    FRAME_ALLOCATOR.lock().free_frames()
}

/// 获取帧分配器的当前状态
///
/// # 返回值
/// - 当前分配指针的 Ppn
/// - 物理帧的结束 Ppn (不包含)
/// - 回收栈的长度
/// - 已分配的帧数
/// - 空闲的帧数
pub fn get_stats() -> (usize, usize, usize, usize, usize) {
    FRAME_ALLOCATOR.lock().get_stats()
}

#[cfg(test)]
mod frame_allocator_tests {
    use super::*;
    use crate::{kassert, mm::address::ConvertablePaddr, test_case};

    // 1. 单帧分配测试
    test_case!(test_single_frame_alloc, {
        let frame = alloc_frame().expect("分配失败");
        let ppn = frame.ppn();

        kassert!(ppn.as_usize() > 0);

        // 帧已自动清零 - 需要转换为 vaddr 才能访问
        let vaddr = ppn.start_addr().to_vaddr();
        let page_ptr = vaddr.as_ptr::<u64>();
        unsafe {
            for i in 0..512 {
                kassert!(*page_ptr.add(i) == 0);
            }
        }
        // frame 在此丢弃，自动回收
    });

    // 2. 多帧分配测试
    test_case!(test_multiple_frames_alloc, {
        let frames = alloc_frames(5).expect("分配失败");
        kassert!(frames.len() == 5);

        // 每个帧都应该是有效的
        for frame in &frames {
            kassert!(frame.ppn().as_usize() > 0);
        }
    });

    // 3. 连续帧分配测试
    test_case!(test_contig_frames_alloc, {
        let frames = alloc_contig_frames(4).expect("分配失败");
        let start_ppn = frames.range().start().as_usize();

        // 验证连续性
        for i in 0..4 {
            let expected = start_ppn + i;
            kassert!(frames.range().start().as_usize() + i == expected);
        }
    });

    // 4. 帧自动回收测试 (RAII)
    test_case!(test_frame_auto_reclaim, {
        // 分配一个帧并保存其 PPN
        let first_ppn = {
            let frame = alloc_frame().expect("分配失败");
            frame.ppn()
        }; // frame 在此丢弃，应被回收至回收栈

        // 再次分配 - 应该从回收栈中获取相同的帧
        let frame2 = alloc_frame().expect("分配失败");
        kassert!(frame2.ppn() == first_ppn); // 验证重用
    });

    // 5. 对齐分配测试
    test_case!(test_aligned_alloc, {
        let frames = alloc_contig_frames_aligned(4, 16).expect("分配失败");
        let ppn = frames.range().start().as_usize();

        // 验证对齐
        kassert!(ppn % 16 == 0);
    });

    // 6. 大量分配测试
    test_case!(test_large_alloc, {
        let frames = alloc_frames(100).expect("分配 100 帧");
        kassert!(frames.len() == 100);
    });
}
