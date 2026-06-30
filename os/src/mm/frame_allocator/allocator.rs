//! 物理内存帧分配器模块
//!
//! 提供了内核用于管理和分配物理内存页帧（Frame）的机制。
//! 采用 RAII (Resource Acquisition Is Initialization) 模式，确保分配的帧
//! 在超出作用域时自动被回收。

use crate::config::PAGE_SIZE;
use crate::mm::address::{ConvertablePA, PageNum, Ppn, PpnRange, UsizeConvert};
use crate::sync::SpinLock;
use alloc::vec::Vec;

const BITS_PER_WORD: usize = u64::BITS as usize;
const MAX_MANAGED_PHYS_BYTES: usize = 8 * 1024 * 1024 * 1024;
const MAX_MANAGED_FRAMES: usize = MAX_MANAGED_PHYS_BYTES / PAGE_SIZE;
const MAX_BITMAP_WORDS: usize = MAX_MANAGED_FRAMES.div_ceil(BITS_PER_WORD);

/// 物理帧跟踪器。
/// 实现了 RAII 模式：当此结构体被 drop 时，它所管理的物理页帧会被自动回收。
#[derive(Debug)]
pub struct FrameTracker(Ppn);

impl FrameTracker {
    /// 创建一个新的 FrameTracker。
    /// 在创建时，会自动将该物理页帧清零。
    pub fn new(ppn: Ppn) -> Self {
        clear_frame(ppn);
        FrameTracker(ppn)
    }

    /// 获取此帧跟踪器所管理的物理页号 (Ppn)。
    pub fn ppn(&self) -> Ppn {
        self.0
    }
}

/// 将指定的物理页帧清零。
fn clear_frame(ppn: Ppn) {
    unsafe {
        // 将 Ppn 转换为虚拟地址指针
        let va = ppn.start_addr().to_va().as_mut_ptr::<u8>();
        // 写入 PAGE_SIZE 字节的 0
        core::ptr::write_bytes(va, 0, PAGE_SIZE);
    }
}

impl Drop for FrameTracker {
    /// 自动回收物理页帧。
    fn drop(&mut self) {
        super::dealloc_frame(self);
    }
}

/// 连续物理帧范围跟踪器。
/// 实现了 RAII 模式：当此结构体被 drop 时，它所管理的物理页帧范围会被自动回收。
#[derive(Debug)]
pub struct FrameRangeTracker {
    range: PpnRange,
}

impl FrameRangeTracker {
    /// 创建一个新的 FrameRangeTracker。
    /// 在创建时，会自动将该范围内的所有物理页帧清零。
    pub fn new(range: PpnRange) -> Self {
        for ppn in range {
            clear_frame(ppn);
        }
        FrameRangeTracker { range }
    }

    /// 获取连续帧范围的起始物理页号 (Ppn)。
    pub fn start_ppn(&self) -> Ppn {
        self.range.start()
    }

    /// 获取连续帧范围的结束物理页号 (Ppn)（不包含）。
    pub fn end_ppn(&self) -> Ppn {
        self.range.end()
    }

    /// 获取连续帧范围内的帧数量。
    pub fn len(&self) -> usize {
        self.range.len()
    }

    /// 获取连续帧范围的引用。
    pub fn range(&self) -> &PpnRange {
        &self.range
    }
}

impl Drop for FrameRangeTracker {
    /// 自动回收连续物理页帧。
    fn drop(&mut self) {
        super::dealloc_contig_frames(self);
    }
}

/// 跟踪的物理帧集合。
/// 用于封装单个或多个不连续的物理帧。
#[derive(Debug)]
pub enum TrackedFrames {
    /// 单个物理帧。
    Single(FrameTracker),
    /// 多个不连续物理帧。
    Multiple(Vec<FrameTracker>),
}

/// 全局物理帧分配器，由自旋锁保护。
pub static FRAME_ALLOCATOR: SpinLock<FrameAllocator> = SpinLock::new(FrameAllocator::new());

/// 物理帧分配器。
/// 采用位图策略跟踪每个物理帧的分配状态。
pub struct FrameAllocator {
    /// 物理帧的起始 Ppn。
    start: Ppn,
    /// 物理帧的结束 Ppn (不包含)。
    end: Ppn,
    /// 位图数据（每个 bit 表示一个帧：0=空闲，1=已分配）。
    bitmap: [u64; MAX_BITMAP_WORDS],
    /// 总帧数。
    total_frames: usize,
    /// 已分配帧数。
    allocated_count: usize,
    /// 上次分配的位置提示，用于加速单帧分配。
    last_alloc_hint: usize,
}

/// 位图帧分配器的实现
impl FrameAllocator {
    /// 创建一个新的帧分配器实例。
    pub const fn new() -> Self {
        FrameAllocator {
            // 使用 usize::MAX 作为初始值，表示未初始化状态
            start: Ppn(usize::MAX),
            end: Ppn(usize::MAX),
            bitmap: [0; MAX_BITMAP_WORDS],
            total_frames: 0,
            allocated_count: 0,
            last_alloc_hint: 0,
        }
    }

    /// 初始化帧分配器，设置可用的物理内存范围。
    pub fn init(&mut self, start: Ppn, end: Ppn) {
        self.start = start;
        self.end = end;
        self.total_frames = end.as_usize() - start.as_usize();
        assert!(
            self.total_frames <= MAX_MANAGED_FRAMES,
            "frame allocator range exceeds bitmap capacity"
        );
        self.bitmap.fill(0);
        self.allocated_count = 0;
        self.last_alloc_hint = 0;
    }

    #[inline]
    fn bitmap_words(&self) -> usize {
        self.total_frames.div_ceil(BITS_PER_WORD)
    }

    /// 检查帧是否空闲。
    #[inline]
    fn is_free(&self, frame_idx: usize) -> bool {
        let word_idx = frame_idx / BITS_PER_WORD;
        let bit_idx = frame_idx % BITS_PER_WORD;
        (self.bitmap[word_idx] & (1u64 << bit_idx)) == 0
    }

    /// 标记帧为已分配。
    #[inline]
    fn mark_allocated(&mut self, frame_idx: usize) {
        let word_idx = frame_idx / BITS_PER_WORD;
        let bit_idx = frame_idx % BITS_PER_WORD;
        self.bitmap[word_idx] |= 1u64 << bit_idx;
    }

    /// 标记帧为空闲。
    #[inline]
    fn mark_free(&mut self, frame_idx: usize) {
        let word_idx = frame_idx / BITS_PER_WORD;
        let bit_idx = frame_idx % BITS_PER_WORD;
        self.bitmap[word_idx] &= !(1u64 << bit_idx);
    }

    /// 分配一个物理帧。
    /// 从 last_alloc_hint 开始循环查找第一个空闲位。
    pub fn alloc_frame(&mut self) -> Option<FrameTracker> {
        let bitmap_words = self.bitmap_words();
        if bitmap_words == 0 {
            return None;
        }

        let start_idx = self.last_alloc_hint;
        for offset in 0..bitmap_words {
            let idx = (start_idx + offset) % bitmap_words;
            let word = self.bitmap[idx];
            if word == u64::MAX {
                continue;
            }

            let bit_pos = (!word).trailing_zeros() as usize;
            let frame_idx = idx * BITS_PER_WORD + bit_pos;
            if frame_idx >= self.total_frames {
                continue;
            }

            self.mark_allocated(frame_idx);
            self.allocated_count += 1;
            self.last_alloc_hint = idx;

            let ppn = self.start + frame_idx;
            return Some(FrameTracker::new(ppn));
        }

        None
    }

    /// 分配指定数量的物理帧（不保证连续）。
    pub fn alloc_frames(&mut self, num: usize) -> Option<Vec<FrameTracker>> {
        let mut frames = Vec::with_capacity(num);
        for _ in 0..num {
            if let Some(frame) = self.alloc_frame() {
                frames.push(frame);
            } else {
                // 分配失败，需要将已分配的帧回收
                // 由于 FrameTracker 实现了 Drop，这里直接 drop frames 即可
                return None;
            }
        }
        Some(frames)
    }

    /// 分配指定数量的**连续**物理帧。
    pub fn alloc_contig_frames(&mut self, num: usize) -> Option<FrameRangeTracker> {
        if num == 0 || num > self.free_frames() {
            return None;
        }

        let mut run_len = 0;
        let mut run_start = 0;

        for frame_idx in 0..self.total_frames {
            if self.is_free(frame_idx) {
                if run_len == 0 {
                    run_start = frame_idx;
                }
                run_len += 1;

                if run_len == num {
                    for i in 0..num {
                        self.mark_allocated(run_start + i);
                    }
                    self.allocated_count += num;

                    let start_ppn = self.start + run_start;
                    let range = PpnRange::from_start_len(start_ppn, num);
                    return Some(FrameRangeTracker::new(range));
                }
            } else {
                run_len = 0;
            }
        }

        None
    }

    /// 分配指定数量的**连续**物理帧，并确保起始地址对齐到 `align_pages` 页的边界。
    pub fn alloc_contig_frames_aligned(
        &mut self,
        num: usize,
        align_pages: usize,
    ) -> Option<FrameRangeTracker> {
        if num == 0 || num > self.free_frames() {
            return None;
        }

        debug_assert!(
            align_pages.is_power_of_two(),
            "Alignment must be power of 2" // 对齐必须是 2 的幂
        );

        let mut frame_idx = 0;
        while frame_idx < self.total_frames {
            let aligned_idx = (frame_idx + align_pages - 1) & !(align_pages - 1);
            if aligned_idx + num > self.total_frames {
                break;
            }

            let mut all_free = true;
            for i in 0..num {
                if !self.is_free(aligned_idx + i) {
                    all_free = false;
                    frame_idx = aligned_idx + i + 1;
                    break;
                }
            }

            if all_free {
                for i in 0..num {
                    self.mark_allocated(aligned_idx + i);
                }
                self.allocated_count += num;

                let start_ppn = self.start + aligned_idx;
                let range = PpnRange::from_start_len(start_ppn, num);
                return Some(FrameRangeTracker::new(range));
            }
        }

        None
    }

    /// 回收一个物理帧。
    pub fn dealloc_frame(&mut self, frame: &FrameTracker) {
        // 检查帧是否在有效范围内
        debug_assert!(
            frame.ppn() >= self.start && frame.ppn() < self.end,
            "dealloc_frame: frame out of range" // 回收帧超出范围
        );

        let ppn = frame.ppn();
        let frame_idx = ppn.as_usize() - self.start.as_usize();

        // 检查帧是否已被分配
        debug_assert!(
            !self.is_free(frame_idx),
            "dealloc_frame: double free detected"
        );

        self.mark_free(frame_idx);
        self.allocated_count -= 1;
    }

    /// 回收一个连续的物理帧范围。
    pub fn dealloc_contig_frames(&mut self, frame_range: &FrameRangeTracker) {
        let start = frame_range.start_ppn();
        let end = frame_range.end_ppn();
        // 检查范围是否在有效范围内
        debug_assert!(
            start >= self.start && end <= self.end,
            "dealloc_contig_frames: frame range out of range" // 回收帧范围超出范围
        );

        let start_idx = start.as_usize() - self.start.as_usize();
        let len = frame_range.len();

        for i in 0..len {
            debug_assert!(
                !self.is_free(start_idx + i),
                "dealloc_contig_frames: double free detected"
            );
            self.mark_free(start_idx + i);
        }
        self.allocated_count -= len;
    }

    /// 获取总的物理帧数
    pub fn total_frames(&self) -> usize {
        self.total_frames
    }

    /// 获取已分配的帧数
    pub fn allocated_frames(&self) -> usize {
        self.allocated_count
    }

    /// 获取空闲的帧数
    pub fn free_frames(&self) -> usize {
        let total = self.total_frames();
        let allocated = self.allocated_frames();
        total - allocated
    }

    /// 获取帧分配器的当前状态
    /// # 返回值
    /// - 当前分配指针的 Ppn
    /// - 物理帧的结束 Ppn (不包含)
    /// - 回收栈的长度（位图实现中恒为 0）
    /// - 已分配的帧数
    /// - 空闲的帧数
    pub fn get_stats(&self) -> (usize, usize, usize, usize, usize) {
        (
            self.start.as_usize() + self.allocated_count,
            self.end.as_usize(),
            0,
            self.allocated_frames(),
            self.free_frames(),
        )
    }
}

impl Default for FrameAllocator {
    fn default() -> Self {
        Self::new()
    }
}
