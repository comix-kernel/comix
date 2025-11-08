#![allow(dead_code)]
use crate::config::PAGE_SIZE;
use crate::mm::address::address::{Address, Paddr, Vaddr};
use crate::mm::address::operations::{AlignOps, CalcOps, UsizeConvert};
use core::ops::Range;

/// trait to represent a page number
pub trait PageNum:
    CalcOps + UsizeConvert + Copy + Clone + PartialEq + PartialOrd + Eq + Ord
{
    type TAddress: Address;

    /// increment the page number by 1
    fn step(&mut self) {
        self.step_by(1);
    }

    /// increment the page number by the given offset
    fn step_by(&mut self, offset: usize) {
        *self = Self::from_usize(self.as_usize() + offset);
    }

    /// decrement the page number by 1
    fn step_back(&mut self) {
        self.step_back_by(1);
    }

    /// decrement the page number by the given offset
    fn step_back_by(&mut self, offset: usize) {
        *self = Self::from_usize(self.as_usize() - offset);
    }

    /// convert an address to a page number (floor)
    fn from_addr_floor(addr: Self::TAddress) -> Self {
        Self::from_usize(addr.align_down_to_page().as_usize() / PAGE_SIZE)
    }

    /// convert an address to a page number (ceil)
    fn from_addr_ceil(addr: Self::TAddress) -> Self {
        Self::from_usize(addr.align_up_to_page().as_usize() / PAGE_SIZE)
    }

    /// get the start address of the page
    fn start_addr(self) -> Self::TAddress {
        Self::TAddress::from_usize(self.as_usize() * PAGE_SIZE)
    }

    /// get the end address of the page
    fn end_addr(self) -> Self::TAddress {
        Self::TAddress::from_usize((self.as_usize() + 1) * PAGE_SIZE)
    }

    /// calculate the difference between two page numbers
    fn diff(self, other: Self) -> isize {
        self.as_usize() as isize - other.as_usize() as isize
    }
}

/// macro to implement the PageNum trait for a type
#[macro_export]
macro_rules! impl_page_num {
    ($type:ty, $addr_type:ty) => {
        impl $crate::mm::address::operations::UsizeConvert for $type {
            fn as_usize(&self) -> usize {
                self.0
            }

            fn from_usize(value: usize) -> Self {
                Self(value)
            }
        }

        $crate::impl_calc_ops!($type);

        impl $crate::mm::address::page_num::PageNum for $type {
            type TAddress = $addr_type;
        }
    };
}

/// physical page number
#[repr(transparent)]
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub struct Ppn(pub usize);
impl_page_num!(Ppn, Paddr);

/// virtual page number
#[repr(transparent)]
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub struct Vpn(pub usize);
impl_page_num!(Vpn, Vaddr);

/// page number range
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PageNumRange<T>
where
    T: PageNum,
{
    pub start: T,
    pub end: T,
}

impl<T> PageNumRange<T>
where
    T: PageNum,
{
    /// create a new page number range
    pub fn new(start: T, end: T) -> Self {
        Self { start, end }
    }

    /// create a page number range from a `Range<T>`
    pub fn from_range(range: Range<T>) -> Self {
        Self {
            start: range.start,
            end: range.end,
        }
    }

    /// create a page number range from start and length
    pub fn from_start_len(start: T, len: usize) -> Self {
        Self {
            start,
            end: T::from_usize(start.as_usize() + len),
        }
    }

    /// get the start page number
    pub fn start(&self) -> T {
        self.start
    }

    /// get the end page number
    pub fn end(&self) -> T {
        self.end
    }

    /// get the length of the range
    pub fn len(&self) -> usize {
        debug_assert!(self.end.as_usize() >= self.start.as_usize());
        self.end.as_usize() - self.start.as_usize()
    }

    /// check if the range is empty
    pub fn empty(&self) -> bool {
        self.start == self.end
    }

    /// check if the range contains a page number
    pub fn contains(&self, addr: T) -> bool {
        addr >= self.start && addr < self.end
    }

    /// check if the range contains another range
    pub fn contains_range(&self, other: &Self) -> bool {
        other.start >= self.start && other.end <= self.end
    }

    /// check if the range is contained in another range
    pub fn contains_in(&self, other: &Self) -> bool {
        self.start >= other.start && self.end <= other.end
    }

    /// check if two ranges overlap
    /// Note: PageNumRange is [start, end), adjacent ranges do NOT overlap
    pub fn overlaps(&self, other: &Self) -> bool {
        !(self.end <= other.start || self.start >= other.end)
    }

    /// get an iterator over the range
    pub fn iter(&self) -> PageNumRangeIterator<T> {
        PageNumRangeIterator {
            range: *self,
            current: self.start,
        }
    }
}

impl<T> IntoIterator for PageNumRange<T>
where
    T: PageNum,
{
    type Item = T;
    type IntoIter = PageNumRangeIterator<T>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

/// iterator for page number range
pub struct PageNumRangeIterator<T>
where
    T: PageNum,
{
    range: PageNumRange<T>,
    current: T,
}

impl<T> Iterator for PageNumRangeIterator<T>
where
    T: PageNum,
{
    type Item = T;

    fn next(&mut self) -> Option<Self::Item> {
        if self.current >= self.range.end {
            return None;
        }
        let result = self.current;
        self.current.step();
        Some(result)
    }
}

pub type PpnRange = PageNumRange<Ppn>;
pub type VpnRange = PageNumRange<Vpn>;

#[cfg(test)]
mod page_num_tests {
    use super::*;
    use crate::mm::address::{Paddr, PageNum, Ppn, Vpn};
    use crate::{kassert, test_case};

    // 1. Ppn/Vpn basic conversion
    test_case!(test_pagenum_from_usize, {
        let ppn = Ppn::from_usize(0x80000);
        kassert!(ppn.as_usize() == 0x80000);

        let vpn = Vpn::from_usize(0x000F_FFFF_FC08_0000);
        kassert!(vpn.as_usize() == 0x000F_FFFF_FC08_0000);
    });

    // 2. Address to page number conversion
    test_case!(test_pagenum_from_addr, {
        let paddr = Paddr::from_usize(0x8000_1234);

        let ppn_floor = Ppn::from_addr_floor(paddr);
        kassert!(ppn_floor.as_usize() == 0x80001); // 0x80001000 >> 12

        let ppn_ceil = Ppn::from_addr_ceil(paddr);
        kassert!(ppn_ceil.as_usize() == 0x80002); // ceil to next page
    });

    // 3. Page number to address
    test_case!(test_pagenum_to_addr, {
        let ppn = Ppn::from_usize(0x80000);

        let start = ppn.start_addr();
        kassert!(start.as_usize() == 0x8000_0000);

        let end = ppn.end_addr();
        kassert!(end.as_usize() == 0x8000_1000);
    });

    // 4. Page number stepping
    test_case!(test_pagenum_step, {
        let mut ppn = Ppn::from_usize(0x80000);

        ppn.step();
        kassert!(ppn.as_usize() == 0x80001);

        ppn.step_back();
        kassert!(ppn.as_usize() == 0x80000);
    });

    // 5. Page number range
    test_case!(test_pagenum_range, {
        let start = Ppn::from_usize(0x80000);
        let end = Ppn::from_usize(0x80003);
        let range = PpnRange::new(start, end);

        kassert!(range.start().as_usize() == 0x80000);
        kassert!(range.end().as_usize() == 0x80003);
        kassert!(range.len() == 3);
    });

    // 6. Page number range iteration
    test_case!(test_pagenum_range_iter, {
        let range = PpnRange::new(Ppn::from_usize(0x80000), Ppn::from_usize(0x80003));

        let mut count = 0;
        for ppn in range {
            kassert!(ppn.as_usize() >= 0x80000);
            kassert!(ppn.as_usize() < 0x80003);
            count += 1;
        }
        kassert!(count == 3);
    });

    // 7. Floor vs Ceil conversion
    test_case!(test_floor_ceil_difference, {
        // Aligned address: floor == ceil
        let aligned = Paddr::from_usize(0x8000_0000);
        let floor1 = Ppn::from_addr_floor(aligned);
        let ceil1 = Ppn::from_addr_ceil(aligned);
        kassert!(floor1.as_usize() == ceil1.as_usize());

        // Unaligned: ceil = floor + 1
        let unaligned = Paddr::from_usize(0x8000_0001);
        let floor2 = Ppn::from_addr_floor(unaligned);
        let ceil2 = Ppn::from_addr_ceil(unaligned);
        kassert!(ceil2.as_usize() == floor2.as_usize() + 1);
    });

    // 8. Page number comparison
    test_case!(test_pagenum_comparison, {
        let ppn1 = Ppn::from_usize(0x80000);
        let ppn2 = Ppn::from_usize(0x80000);
        let ppn3 = Ppn::from_usize(0x80001);

        kassert!(ppn1 == ppn2);
        kassert!(ppn1 < ppn3);
        kassert!(ppn3 > ppn1);
    });
}
