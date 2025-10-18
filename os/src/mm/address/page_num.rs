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

    /// create a page number range from a Range<T>
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
