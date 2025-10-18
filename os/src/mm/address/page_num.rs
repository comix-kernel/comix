use crate::config::PAGE_SIZE;
use crate::mm::address::address::{Address, Vaddr, Paddr};
use crate::mm::address::operations::{AlignOps, CalcOps, UsizeConvert};

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