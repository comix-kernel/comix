use crate::mm::address::operations::{AlignOps, CalcOps, UsizeConvert};
use core::mem::size_of;
use core::ops::Range;

// trait to represent an address
pub trait Address:
    CalcOps + AlignOps + UsizeConvert + Copy + Clone + PartialEq + PartialOrd + Eq + Ord
{
    fn is_null(self) -> bool {
        self.as_usize() == 0
    }

    fn null() -> Self {
        Self::from_usize(0)
    }

    fn page_offset(self) -> usize {
        self.as_usize() & (crate::config::PAGE_SIZE - 1)
    }

    fn addr_diff(self, other: Self) -> isize {
        self.as_usize() as isize - other.as_usize() as isize
    }

    fn add<T>(self) -> Self {
        self.add_by(size_of::<T>())
    }

    fn add_n<T>(self, n: usize) -> Self {
        self.add_by(size_of::<T>() * n)
    }

    fn add_by(self, offset: usize) -> Self {
        Self::from_usize(self.as_usize() + offset)
    }

    fn sub(self) -> Self {
        self.sub_by(size_of::<Self>())
    }

    fn sub_n(self, n: usize) -> Self {
        self.sub_by(size_of::<Self>() * n)
    }

    fn sub_by(self, offset: usize) -> Self {
        Self::from_usize(self.as_usize() - offset)
    }

    fn step(&mut self) {
        self.step_by(size_of::<Self>())
    }

    fn step_n(&mut self, n: usize) {
        self.step_by(size_of::<Self>() * n)
    }

    fn step_back(&mut self) {
        self.step_back_by(size_of::<Self>())
    }

    fn step_back_n(&mut self, n: usize) {
        self.step_back_by(size_of::<Self>() * n)
    }

    fn step_by(&mut self, offset: usize) {
        *self = self.add_by(offset);
    }

    fn step_back_by(&mut self, offset: usize) {
        *self = self.sub_by(offset);
    }
}

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

// physical address
#[repr(transparent)]
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Paddr(*const ());
impl_address!(Paddr);

// virtual address
#[repr(transparent)]
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Vaddr(*const ());
impl_address!(Vaddr);

impl Vaddr {
    // from_ref
    pub fn from_ref<T>(r: &T) -> Self {
        Self::from_ptr(r as *const T)
    }

    // from_ptr
    pub fn from_ptr<T>(p: *const T) -> Self {
        Self::from_usize(p as usize)
    }

    // as_ref
    // the caller must ensure that the address is valid for type T
    pub unsafe fn as_ref<T>(&self) -> &T {
        &*(self.as_usize() as *const T)
    }

    // as_mut
    // the caller must ensure that the address is valid for type T
    pub unsafe fn as_mut<T>(&mut self) -> &mut T {
        &mut *(self.as_usize() as *mut T)
    }

    // as_ptr
    pub fn as_ptr<T>(&self) -> *const T {
        self.as_usize() as *const T
    }

    // as_mut_ptr
    // the caller must ensure that the address is valid for type T
    pub unsafe fn as_mut_ptr<T>(&mut self) -> *mut T {
        self.as_usize() as *mut T
    }
}
