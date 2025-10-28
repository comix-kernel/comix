#![allow(dead_code)]
use crate::mm::address::operations::{AlignOps, CalcOps, UsizeConvert};
use core::mem::size_of;
use core::ops::Range;

/// trait to represent an address
pub trait Address:
    CalcOps + AlignOps + UsizeConvert + Copy + Clone + PartialEq + PartialOrd + Eq + Ord
{
    /// check if the address is null (zero)
    fn is_null(self) -> bool {
        self.as_usize() == 0
    }

    /// return a null address (zero)
    fn null() -> Self {
        Self::from_usize(0)
    }

    /// get the offset within a page
    fn page_offset(self) -> usize {
        self.as_usize() & (crate::config::PAGE_SIZE - 1)
    }

    /// calculate the difference between two addresses
    fn addr_diff(self, other: Self) -> isize {
        self.as_usize() as isize - other.as_usize() as isize
    }

    /// add the size of type T to the address
    fn add<T>(self) -> Self {
        self.add_by(size_of::<T>())
    }

    /// add the size of n elements of type T to the address
    fn add_n<T>(self, n: usize) -> Self {
        self.add_by(size_of::<T>() * n)
    }

    /// add an offset to the address
    fn add_by(self, offset: usize) -> Self {
        Self::from_usize(self.as_usize() + offset)
    }

    /// subtract the size of Self from the address
    fn sub(self) -> Self {
        self.sub_by(size_of::<Self>())
    }

    /// subtract the size of n elements of Self from the address
    fn sub_n(self, n: usize) -> Self {
        self.sub_by(size_of::<Self>() * n)
    }

    /// subtract an offset from the address
    fn sub_by(self, offset: usize) -> Self {
        Self::from_usize(self.as_usize() - offset)
    }

    /// increment the address by the size of Self
    fn step(&mut self) {
        self.step_by(size_of::<Self>())
    }

    /// increment the address by the size of n elements of Self
    fn step_n(&mut self, n: usize) {
        self.step_by(size_of::<Self>() * n)
    }

    /// decrement the address by the size of Self
    fn step_back(&mut self) {
        self.step_back_by(size_of::<Self>())
    }

    /// decrement the address by the size of n elements of Self
    fn step_back_n(&mut self, n: usize) {
        self.step_back_by(size_of::<Self>() * n)
    }

    /// increment the address by the given offset
    fn step_by(&mut self, offset: usize) {
        *self = self.add_by(offset);
    }

    /// decrement the address by the given offset
    fn step_back_by(&mut self, offset: usize) {
        *self = self.sub_by(offset);
    }
}

/// macro to implement the Address trait for a type
#[macro_export]
macro_rules! impl_address {
    ($type:ty) => {
        impl $crate::mm::address::operations::UsizeConvert for $type {
            fn as_usize(&self) -> usize {
                unsafe { core::mem::transmute::<Self, usize>(*self) }
            }
            fn from_usize(value: usize) -> Self {
                unsafe { core::mem::transmute::<usize, Self>(value) }
            }
        }

        $crate::impl_calc_ops!($type);
        impl $crate::mm::address::operations::AlignOps for $type {}

        impl $crate::mm::address::address::Address for $type {}

        unsafe impl Sync for $type {}
        unsafe impl Send for $type {}
    };
}

/// trait for converting physical addresses to virtual addresses
pub trait ConvertablePaddr {
    /// check if the address is a valid physical address
    fn is_valid_paddr(&self) -> bool;
    /// convert physical address to virtual address
    fn to_vaddr(&self) -> Vaddr;
}

/// physical address
#[repr(transparent)]
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Paddr(*const ());
impl_address!(Paddr);

impl ConvertablePaddr for Paddr {
    fn is_valid_paddr(&self) -> bool {
        self.as_usize() == unsafe { crate::arch::mm::vaddr_to_paddr(self.as_usize()) }
    }

    fn to_vaddr(&self) -> Vaddr {
        Vaddr::from_usize(crate::arch::mm::paddr_to_vaddr(self.as_usize()))
    }
}

/// trait for converting virtual addresses to physical addresses
pub trait ConvertableVaddr {
    /// check if the address is a valid virtual address
    fn is_valid_vaddr(&self) -> bool;
    /// convert virtual address to physical address
    fn to_paddr(&self) -> Paddr;
}

/// virtual address
#[repr(transparent)]
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Vaddr(*const ());
impl_address!(Vaddr);

impl ConvertableVaddr for Vaddr {
    fn is_valid_vaddr(&self) -> bool {
        self.as_usize() == crate::arch::mm::paddr_to_vaddr(self.as_usize())
    }

    fn to_paddr(&self) -> Paddr {
        Paddr::from_usize(unsafe { crate::arch::mm::vaddr_to_paddr(self.as_usize()) })
    }
}

impl Vaddr {
    /// create a virtual address from a reference
    pub fn from_ref<T>(r: &T) -> Self {
        Self::from_ptr(r as *const T)
    }

    /// create a virtual address from a pointer
    pub fn from_ptr<T>(p: *const T) -> Self {
        Self::from_usize(p as usize)
    }

    /// convert the address to a reference
    /// # Safety
    /// the caller must ensure that the address is valid for type T
    pub unsafe fn as_ref<T>(&self) -> &T {
        unsafe { &*(self.as_usize() as *const T) }
    }

    /// convert the address to a mutable reference
    /// # Safety
    /// the caller must ensure that the address is valid for type T
    pub unsafe fn as_mut<T>(&mut self) -> &mut T {
        unsafe { &mut *(self.as_usize() as *mut T) }
    }

    /// convert the address to a const pointer
    pub fn as_ptr<T>(&self) -> *const T {
        self.as_usize() as *const T
    }

    /// convert the address to a mutable pointer
    /// # Safety
    /// the caller must ensure that the address is valid for type T
    pub unsafe fn as_mut_ptr<T>(&mut self) -> *mut T {
        self.as_usize() as *mut T
    }
}

/// address range
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AddressRange<T>
where
    T: Address,
{
    start: T,
    end: T,
}

impl<T> AddressRange<T>
where
    T: Address,
{
    /// create a new address range
    pub fn new(start: T, end: T) -> Self {
        Self { start, end }
    }

    /// create an address range from a Range<T>
    pub fn from_range(range: Range<T>) -> Self {
        Self {
            start: range.start,
            end: range.end,
        }
    }

    /// create an address range from start address and length
    pub fn from_start_len(start: T, len: usize) -> Self {
        Self {
            start,
            end: T::from_usize(start.as_usize() + len),
        }
    }

    /// create an address range from a slice
    pub fn from_slices(slices: &[T]) -> Option<Self> {
        if slices.len() < 2 {
            return None;
        }
        Some(Self {
            start: slices[0],
            end: slices[slices.len() - 1],
        })
    }

    /// get the start address
    pub fn start(&self) -> T {
        self.start
    }

    /// get the end address
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

    /// check if the range contains an address
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

    /// check if the range intersects with another range
    pub fn intersects(&self, other: &Self) -> bool {
        self.start < other.end && other.start < self.end
    }

    /// check if the range is adjacent to another range
    pub fn adjacent(&self, other: &Self) -> bool {
        self.end == other.start || other.end == self.start
    }

    /// get the intersection of two ranges
    pub fn intersection(&self, other: &Self) -> Option<Self> {
        if !self.intersects(other) {
            return None;
        }
        let start = core::cmp::max(self.start, other.start);
        let end = core::cmp::min(self.end, other.end);
        Some(Self { start, end })
    }

    /// get the union of two ranges
    pub fn union(&self, other: &Self) -> Option<Self> {
        if !self.intersects(other) && !self.adjacent(other) {
            return None;
        }
        let start = core::cmp::min(self.start, other.start);
        let end = core::cmp::max(self.end, other.end);
        Some(Self { start, end })
    }

    /// get an iterator over the range
    pub fn iter(&self) -> AddressRangeIterator<T> {
        AddressRangeIterator {
            range: *self,
            current: self.start,
        }
    }
}

impl<T> IntoIterator for AddressRange<T>
where
    T: Address,
{
    type Item = T;
    type IntoIter = AddressRangeIterator<T>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

/// iterator for address range
pub struct AddressRangeIterator<T>
where
    T: Address,
{
    range: AddressRange<T>,
    current: T,
}

impl<T> Iterator for AddressRangeIterator<T>
where
    T: Address,
{
    type Item = T;

    fn next(&mut self) -> Option<Self::Item> {
        if self.current >= self.range.end {
            return None;
        }
        let addr = self.current;
        self.current.step();
        Some(addr)
    }
}

/// physical address range
pub type PaddrRange = AddressRange<Paddr>;

/// virtual address range
pub type VaddrRange = AddressRange<Vaddr>;

#[cfg(test)]
mod address_basic_tests {
    use super::*;
    use crate::arch::mm::paddr_to_vaddr;
    use crate::{test_case, kassert};

    // 1.1 Paddr/Vaddr creation and conversion
    test_case!(test_address_roundtrip, {
        let test_values = [0x0, 0x1000, 0x8000_0000, 0x8000_1234];

        for &val in &test_values {
            let paddr = Paddr::from_usize(val);
            kassert!(paddr.as_usize() == val);

            let vaddr = Vaddr::from_usize(val);
            kassert!(vaddr.as_usize() == val);
        }
    });

    // 1.2 Null address
    test_case!(test_null_address, {
        let paddr = Paddr::null();
        kassert!(paddr.is_null());
        kassert!(paddr.as_usize() == 0);
    });

    // 1.3 Page offset
    test_case!(test_page_offset, {
        let cases = [(0x8000_0000, 0), (0x8000_0123, 0x123), (0x8000_0FFF, 0xFFF)];
        for &(addr, expected) in &cases {
            kassert!(Paddr::from_usize(addr).page_offset() == expected);
        }
    });

    // 1.4 Paddr ↔ Vaddr conversion
    test_case!(test_paddr_vaddr_conversion, {
        let paddrs = [0x8000_0000, 0x8000_1000, 0x8020_0000];

        for &paddr_val in &paddrs {
            let paddr = Paddr::from_usize(paddr_val);
            let vaddr = paddr.to_vaddr();
            let back = vaddr.to_paddr();
            kassert!(back.as_usize() == paddr_val);
            kassert!(vaddr.as_usize() == paddr_to_vaddr(paddr_val));
        }
    });

    // 1.5 Address comparison
    test_case!(test_address_comparison, {
        let a1 = Paddr::from_usize(0x8000_0000);
        let a2 = Paddr::from_usize(0x8000_0000);
        let a3 = Paddr::from_usize(0x8000_1000);

        kassert!(a1 == a2);
        kassert!(a1 < a3);
        kassert!(a3 > a1);
    });
}