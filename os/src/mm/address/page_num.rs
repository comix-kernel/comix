use crate::config::PAGE_SIZE;
use crate::mm::address::address::{Address, Paddr, Vaddr};
use crate::mm::address::operations::{AlignOps, CalcOps, UsizeConvert};
use core::ops::Range;

// trait to represent an page number
pub trait PageNum:
    CalcOps + UsizeConvert + Copy + Clone + PartialEq + PartialOrd + Eq + Ord
{
    type TAddress: Address;

    fn step(&mut self) {
        self.step_by(1);
    }

    fn step_by(&mut self, offset: usize) {
        *self = Self::from_usize(self.as_usize() + offset);
    }

    fn step_back(&mut self) {
        self.step_back_by(1);
    }

    fn step_back_by(&mut self, offset: usize) {
        *self = Self::from_usize(self.as_usize() - offset);
    }

    fn from_addr_floor(addr: Self::TAddress) -> Self {
        Self::from_usize(addr.align_down_to_page().as_usize() / PAGE_SIZE)
    }

    fn from_addr_ceil(addr: Self::TAddress) -> Self {
        Self::from_usize(addr.align_up_to_page().as_usize() / PAGE_SIZE)
    }

    fn start_addr(self) -> Self::TAddress {
        Self::TAddress::from_usize(self.as_usize() * PAGE_SIZE)
    }

    fn end_addr(self) -> Self::TAddress {
        Self::TAddress::from_usize((self.as_usize() + 1) * PAGE_SIZE)
    }

    fn diff(self, other: Self) -> isize {
        self.as_usize() as isize - other.as_usize() as isize
    }
}

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

// phyical page number
#[repr(transparent)]
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub struct Ppn(pub usize);
impl_page_num!(Ppn, Paddr);

// virtual page number
#[repr(transparent)]
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub struct Vpn(pub usize);
impl_page_num!(Vpn, Vaddr);

// trait to represent a range of page numbers
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PageNumRange<T>
where
    T: PageNum,
{
    pub start: T,
    pub end: T,
}

// TODO: implement methods for PageNumRange
impl<T> PageNumRange<T>
where
    T: PageNum,
{
    pub fn new(start: T, end: T) -> Self {
        Self { start, end }
    }

    pub fn from_range(range: Range<T>) -> Self {
        Self {
            start: range.start,
            end: range.end,
        }
    }

    pub fn from_start_len(start: T, len: usize) -> Self {
        Self {
            start,
            end: T::from_usize(start.as_usize() + len),
        }
    }

    pub fn len(&self) -> usize {
        debug_assert!(self.end.as_usize() >= self.start.as_usize());
        self.end.as_usize() - self.start.as_usize()
    }

    pub fn empty(&self) -> bool {
        self.start == self.end
    }

    pub fn contains(&self, addr: T) -> bool {
        addr >= self.start && addr < self.end
    }

    pub fn contains_range(&self, other: &Self) -> bool {
        other.start >= self.start && other.end <= self.end
    }

    pub fn contains_in(&self, other: &Self) -> bool {
        self.start >= other.start && self.end <= other.end
    }

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

// iterator for PageNumRange
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
