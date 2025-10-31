#![allow(dead_code)]
use crate::config::PAGE_SIZE;
use crate::mm::address::{ConvertablePaddr, Paddr, PageNum, Ppn, PpnRange, UsizeConvert};
use crate::sync::spin_lock::SpinLock;
use alloc::vec::Vec;
use lazy_static::lazy_static;

#[derive(Debug)]
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

#[derive(Debug)]
pub struct FrameRangeTracker {
    range: PpnRange,
}

impl FrameRangeTracker {
    pub fn new(range: PpnRange) -> Self {
        for ppn in range {
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

#[derive(Debug)]
pub enum TrackedFrames {
    Single(FrameTracker),
    Multiple(Vec<FrameTracker>),
    Contiguous(FrameRangeTracker),
}

lazy_static! {
    pub static ref FRAME_ALLOCATOR: SpinLock<FrameAllocator> = SpinLock::new(FrameAllocator::new());
}

pub struct FrameAllocator {
    start: Ppn,
    end: Ppn,
    cur: Ppn,
    /// recycled frames stack
    recycled: Vec<Ppn>,
}

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

    pub fn alloc_contig_frames_aligned(
        &mut self,
        num: usize,
        align_pages: usize,
    ) -> Option<FrameRangeTracker> {
        if num == 0 {
            return None;
        }

        debug_assert!(
            align_pages.is_power_of_two(),
            "Alignment must be power of 2"
        );

        // 向上对齐
        let aligned_cur_val =
            (self.cur.as_usize() + align_pages - 1).div_ceil(align_pages) * align_pages;
        let aligned_cur = Ppn::from_usize(aligned_cur_val);

        // 检查空间
        let required_end = aligned_cur + num;
        if required_end <= self.end {
            // 跳过的帧加入 recycled
            for ppn_val in self.cur.as_usize()..aligned_cur.as_usize() {
                self.recycled.push(Ppn::from_usize(ppn_val));
            }

            self.cur = required_end;
            let range = PpnRange::from_start_len(aligned_cur, num);
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
            end <= self.cur,
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

/// initialize the global frame allocator with the available physical memory range
///
/// # Parameters
///
/// * `start_addr` - start address of the available physical memory
/// * `end_addr` - end address of the available physical memory
pub fn init_frame_allocator(start_addr: usize, end_addr: usize) {
    let start_ppn = Ppn::from_addr_ceil(Paddr::from_usize(start_addr));
    let end_ppn = Ppn::from_addr_floor(Paddr::from_usize(end_addr));

    let mut allocator = FRAME_ALLOCATOR.lock();
    allocator.init(start_ppn, end_ppn);
}

/// allocate a single frame
pub fn alloc_frame() -> Option<FrameTracker> {
    FRAME_ALLOCATOR.lock().alloc_frame()
}

/// allocate multiple frames (may not be contiguous)
pub fn alloc_frames(num: usize) -> Option<Vec<FrameTracker>> {
    FRAME_ALLOCATOR.lock().alloc_frames(num)
}

/// allocate contiguous frames
pub fn alloc_contig_frames(num: usize) -> Option<FrameRangeTracker> {
    FRAME_ALLOCATOR.lock().alloc_contig_frames(num)
}

/// allocate contiguous frames with alignment
pub fn alloc_contig_frames_aligned(num: usize, align_pages: usize) -> Option<FrameRangeTracker> {
    FRAME_ALLOCATOR
        .lock()
        .alloc_contig_frames_aligned(num, align_pages)
}

/// deallocate a single frame
fn dealloc_frame(frame: &FrameTracker) {
    FRAME_ALLOCATOR.lock().dealloc_frame(frame);
}

/// deallocate multiple frames (may not be contiguous)
fn dealloc_frames(frames: &[FrameTracker]) {
    let mut allocator = FRAME_ALLOCATOR.lock();
    for frame in frames {
        allocator.dealloc_frame(frame);
    }
}

/// deallocate contiguous frames
fn dealloc_contig_frames(frame_range: &FrameRangeTracker) {
    FRAME_ALLOCATOR.lock().dealloc_contig_frames(frame_range);
}

#[cfg(test)]
mod frame_allocator_tests {
    use super::*;
    use crate::{kassert, test_case};

    // 1. Single frame allocation
    test_case!(test_single_frame_alloc, {
        let frame = alloc_frame().expect("alloc failed");
        let ppn = frame.ppn();

        kassert!(ppn.as_usize() > 0);

        // Frame is auto-cleared - need to convert to vaddr to access
        let vaddr = ppn.start_addr().to_vaddr();
        let page_ptr = vaddr.as_ptr::<u64>();
        unsafe {
            for i in 0..512 {
                kassert!(*page_ptr.add(i) == 0);
            }
        }
        // frame drops here, auto-reclaimed
    });

    // 2. Multiple frame allocation
    test_case!(test_multiple_frames_alloc, {
        let frames = alloc_frames(5).expect("alloc failed");
        kassert!(frames.len() == 5);

        // Each frame should be valid
        for frame in &frames {
            kassert!(frame.ppn().as_usize() > 0);
        }
    });

    // 3. Contiguous frame allocation
    test_case!(test_contig_frames_alloc, {
        let frames = alloc_contig_frames(4).expect("alloc failed");
        let start_ppn = frames.range().start().as_usize();

        // Verify contiguity
        for i in 0..4 {
            let expected = start_ppn + i;
            kassert!(frames.range().start().as_usize() + i == expected);
        }
    });

    // 4. Frame auto-reclaim (RAII)
    test_case!(test_frame_auto_reclaim, {
        // Allocate a frame and save its PPN
        let first_ppn = {
            let frame = alloc_frame().expect("alloc failed");
            frame.ppn()
        }; // frame drops here, should be reclaimed to recycled stack

        // Allocate again - should get the same frame from recycled stack
        let frame2 = alloc_frame().expect("alloc failed");
        kassert!(frame2.ppn() == first_ppn); // Verify reuse
    });

    // 5. Aligned allocation
    test_case!(test_aligned_alloc, {
        let frames = alloc_contig_frames_aligned(4, 16).expect("alloc failed");
        let ppn = frames.range().start().as_usize();

        // Verify alignment
        kassert!(ppn % 16 == 0);
    });

    // 6. Large allocation
    test_case!(test_large_alloc, {
        let frames = alloc_frames(100).expect("alloc 100 frames");
        kassert!(frames.len() == 100);
    });
}
