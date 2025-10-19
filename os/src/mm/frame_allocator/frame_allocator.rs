use core::ptr::{addr_of, addr_of_mut};
use core::usize;

use crate::config::PAGE_SIZE;
use crate::mm::address::{ConvertablePaddr, Paddr, PageNum, Ppn, PpnRange, UsizeConvert};
use alloc::vec::Vec;

pub struct FrameTracker(Ppn);

impl FrameTracker {
    pub fn new(ppn: Ppn) -> Self {
        clear_frame(ppn);
        FrameTracker(ppn)
    }

    pub fn ppn(&self) -> Ppn {
        self.0
    }
}

fn clear_frame(ppn: Ppn) {
    unsafe {
        let va = ppn.start_addr().to_vaddr().as_mut_ptr::<u8>();
        core::ptr::write_bytes(va, 0, PAGE_SIZE);
    }
}

impl Drop for FrameTracker {
    fn drop(&mut self) {
        dealloc_frame(self);
    }
}

pub struct FrameRangeTracker {
    range: PpnRange,
}

impl FrameRangeTracker {
    pub fn new(range: PpnRange) -> Self {
        for ppn in range.clone() {
            clear_frame(ppn);
        }
        FrameRangeTracker { range }
    }

    pub fn start_ppn(&self) -> Ppn {
        self.range.start()
    }

    pub fn end_ppn(&self) -> Ppn {
        self.range.end()
    }

    pub fn len(&self) -> usize {
        self.range.len()
    }

    pub fn range(&self) -> &PpnRange {
        &self.range
    }
}

impl Drop for FrameRangeTracker {
    fn drop(&mut self) {
        dealloc_contig_frames(self);
    }
}

/// TODO: replace with proper synchronization primitive
///
/// global frame allocator instance
///
/// use static mut in single-core environment without multitasking
static mut FRAME_ALLOCATOR: Option<FrameAllocator> = None;

struct FrameAllocator {
    start: Ppn,
    end: Ppn,
    cur: Ppn,
    /// recycled frames stack
    recycled: Vec<Ppn>,
}

// TODO: implement FrameAllocator
/// lazy frame allocator
impl FrameAllocator {
    pub fn new() -> Self {
        FrameAllocator {
            start: Ppn::from_usize(usize::MAX),
            end: Ppn::from_usize(usize::MAX),
            cur: Ppn::from_usize(usize::MAX),
            recycled: Vec::new(),
        }
    }

    pub fn init(&mut self, start: Ppn, end: Ppn) {
        self.start = start;
        self.end = end;
        self.cur = start;
    }

    pub fn alloc_frame(&mut self) -> Option<FrameTracker> {
        if let Some(ppn) = self.recycled.pop() {
            Some(FrameTracker::new(ppn))
        } else if self.cur < self.end {
            let ppn = self.cur;
            self.cur.step();
            Some(FrameTracker::new(ppn))
        } else {
            None
        }
    }

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

    pub fn alloc_contig_frames(&mut self, num: usize) -> Option<FrameRangeTracker> {
        if num == 0 {
            return None;
        }

        // 检查是否有足够的连续帧
        let required_end = self.cur + num;
        if required_end <= self.end {
            let start = self.cur;
            self.cur = required_end;
            let range = PpnRange::from_start_len(start, num);
            Some(FrameRangeTracker::new(range))
        } else {
            None
        }
    }

    pub fn dealloc_frame(&mut self, frame: &FrameTracker) {
        // is valid
        debug_assert!(
            frame.ppn() >= self.start && frame.ppn() < self.end,
            "dealloc_frame: frame out of range"
        );
        // is allocated
        debug_assert!(
            frame.ppn() < self.cur && self.recycled.iter().all(|&ppn| ppn != frame.ppn()),
        );

        let ppn = frame.ppn();
        self.recycled.push(ppn);
        self.recycled.sort_unstable();

        if let Some(&last) = self.recycled.last() {
            // 回收栈顶部的帧是当前分配指针前面的连续帧
            if last + 1 == self.cur {
                // 回收连续帧
                let mut new_cur = last;
                self.recycled.pop();
                while let Some(&top) = self.recycled.last() {
                    if top + 1 == new_cur {
                        new_cur = top;
                        self.recycled.pop();
                    } else {
                        break;
                    }
                }
                self.cur = new_cur;
            }
        }
    }

    pub fn dealloc_contig_frames(&mut self, frame_range: &FrameRangeTracker) {
        let start = frame_range.start_ppn();
        let end = frame_range.end_ppn();
        // is valid
        debug_assert!(
            start >= self.start && end <= self.end,
            "dealloc_contig_frames: frame range out of range"
        );
        // is allocated
        debug_assert!(
            end < self.cur,
            "dealloc_contig_frames: frame range not allocated"
        );

        for ppn in frame_range.range().into_iter() {
            self.recycled.push(ppn);
        }
        self.recycled.sort_unstable();

        if let Some(&last) = self.recycled.last() {
            // 回收栈顶部的帧是当前分配指针前面的连续帧
            if last + 1 == self.cur {
                // 回收连续帧
                let mut new_cur = last;
                self.recycled.pop();
                while let Some(&top) = self.recycled.last() {
                    if top + 1 == new_cur {
                        new_cur = top;
                        self.recycled.pop();
                    } else {
                        break;
                    }
                }
                self.cur = new_cur;
            }
        }
    }
}

/// TODO: replace with proper synchronization primitive
///
/// initialize the global frame allocator while booting
pub fn init_frame_allocator(start_addr: usize, end_addr: usize) {
    let start_ppn = Ppn::from_addr_ceil(Paddr::from_usize(start_addr));
    let end_ppn = Ppn::from_addr_floor(Paddr::from_usize(end_addr));

    unsafe {
        let allocator_ptr = addr_of_mut!(FRAME_ALLOCATOR);
        if (*allocator_ptr).is_none() {
            let mut allocator = FrameAllocator::new();
            allocator.init(start_ppn, end_ppn);
            *allocator_ptr = Some(allocator);
        }
    }
}

/// allocate a single frame
pub fn alloc_frame() -> Option<FrameTracker> {
    unsafe { (*addr_of_mut!(FRAME_ALLOCATOR)).as_mut()?.alloc_frame() }
}

/// allocate multiple frames (may not be contiguous)
pub fn alloc_frames(num: usize) -> Option<Vec<FrameTracker>> {
    unsafe { (*addr_of_mut!(FRAME_ALLOCATOR)).as_mut()?.alloc_frames(num) }
}

/// allocate contiguous frames
pub fn alloc_contig_frames(num: usize) -> Option<FrameRangeTracker> {
    unsafe {
        (*addr_of_mut!(FRAME_ALLOCATOR))
            .as_mut()?
            .alloc_contig_frames(num)
    }
}

/// deallocate a single frame
fn dealloc_frame(frame: &FrameTracker) {
    unsafe {
        if let Some(allocator) = (*addr_of_mut!(FRAME_ALLOCATOR)).as_mut() {
            allocator.dealloc_frame(frame);
        }
    }
}

/// deallocate multiple frames (may not be contiguous)
fn dealloc_frames(frames: &[FrameTracker]) {
    unsafe {
        if let Some(allocator) = (*addr_of_mut!(FRAME_ALLOCATOR)).as_mut() {
            for frame in frames {
                allocator.dealloc_frame(frame);
            }
        }
    }
}

/// deallocate contiguous frames
fn dealloc_contig_frames(frame_range: &FrameRangeTracker) {
    unsafe {
        if let Some(allocator) = (*addr_of_mut!(FRAME_ALLOCATOR)).as_mut() {
            allocator.dealloc_contig_frames(frame_range);
        }
    }
}
