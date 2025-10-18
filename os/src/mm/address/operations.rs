use crate::config::PAGE_SIZE;
use core::ops::{
    Add, AddAssign, BitAnd, BitAndAssign, BitOr, BitOrAssign, BitXor, BitXorAssign, Shl, ShlAssign,
    Shr, ShrAssign, Sub, SubAssign,
};

// convert between usize and the type
pub trait UsizeConvert: Copy + Clone + PartialEq + PartialOrd + Eq + Ord {
    fn as_usize(&self) -> usize;
    fn from_usize(value: usize) -> Self;
}

// arithmetic and bitwise operations
pub trait CalcOps:
    UsizeConvert
    + Add<usize>
    + Add<Self>
    + AddAssign<usize>
    + AddAssign<Self>
    + Sub<usize>
    + Sub<Self>
    + SubAssign<usize>
    + SubAssign<Self>
    + BitAnd<usize>
    + BitAnd<Self>
    + BitAndAssign<usize>
    + BitAndAssign<Self>
    + BitOr<usize>
    + BitOr<Self>
    + BitOrAssign<usize>
    + BitOrAssign<Self>
    + BitXor<usize>
    + BitXor<Self>
    + BitXorAssign<usize>
    + BitXorAssign<Self>
    + Shl<usize>
    + ShlAssign<usize>
    + Shr<usize>
    + ShrAssign<usize>
{
}

// macro to implement arithmetic and bitwise operations
#[macro_export]
macro_rules! impl_calc_ops {
    ($type:ty) => {
        impl core::ops::Add<usize> for $type {
            type Output = Self;
            fn add(self, rhs: usize) -> Self::Output {
                $crate::mm::address::operations::UsizeConvert::from_usize(self.as_usize() + rhs)
            }
        }
        impl core::ops::Add<Self> for $type {
            type Output = Self;
            fn add(self, rhs: Self) -> Self::Output {
                $crate::mm::address::operations::UsizeConvert::from_usize(
                    self.as_usize() + rhs.as_usize(),
                )
            }
        }
        impl core::ops::AddAssign<usize> for $type {
            fn add_assign(&mut self, rhs: usize) {
                *self =
                    $crate::mm::address::operations::UsizeConvert::from_usize(self.as_usize() + rhs)
            }
        }
        impl core::ops::AddAssign<Self> for $type {
            fn add_assign(&mut self, rhs: Self) {
                *self = $crate::mm::address::operations::UsizeConvert::from_usize(
                    self.as_usize() + rhs.as_usize(),
                )
            }
        }
        impl core::ops::Sub<usize> for $type {
            type Output = Self;
            fn sub(self, rhs: usize) -> Self::Output {
                $crate::mm::address::operations::UsizeConvert::from_usize(self.as_usize() - rhs)
            }
        }
        impl core::ops::Sub<Self> for $type {
            type Output = Self;
            fn sub(self, rhs: Self) -> Self::Output {
                $crate::mm::address::operations::UsizeConvert::from_usize(
                    self.as_usize() - rhs.as_usize(),
                )
            }
        }
        impl core::ops::SubAssign<usize> for $type {
            fn sub_assign(&mut self, rhs: usize) {
                *self =
                    $crate::mm::address::operations::UsizeConvert::from_usize(self.as_usize() - rhs)
            }
        }
        impl core::ops::SubAssign<Self> for $type {
            fn sub_assign(&mut self, rhs: Self) {
                *self = $crate::mm::address::operations::UsizeConvert::from_usize(
                    self.as_usize() - rhs.as_usize(),
                )
            }
        }
        impl core::ops::BitAnd<usize> for $type {
            type Output = Self;
            fn bitand(self, rhs: usize) -> Self::Output {
                $crate::mm::address::operations::UsizeConvert::from_usize(self.as_usize() & rhs)
            }
        }
        impl core::ops::BitAnd<Self> for $type {
            type Output = Self;
            fn bitand(self, rhs: Self) -> Self::Output {
                $crate::mm::address::operations::UsizeConvert::from_usize(
                    self.as_usize() & rhs.as_usize(),
                )
            }
        }
        impl core::ops::BitAndAssign<usize> for $type {
            fn bitand_assign(&mut self, rhs: usize) {
                *self =
                    $crate::mm::address::operations::UsizeConvert::from_usize(self.as_usize() & rhs)
            }
        }
        impl core::ops::BitAndAssign<Self> for $type {
            fn bitand_assign(&mut self, rhs: Self) {
                *self = $crate::mm::address::operations::UsizeConvert::from_usize(
                    self.as_usize() & rhs.as_usize(),
                )
            }
        }
        impl core::ops::BitOr<usize> for $type {
            type Output = Self;
            fn bitor(self, rhs: usize) -> Self::Output {
                $crate::mm::address::operations::UsizeConvert::from_usize(self.as_usize() | rhs)
            }
        }
        impl core::ops::BitOr<Self> for $type {
            type Output = Self;
            fn bitor(self, rhs: Self) -> Self::Output {
                $crate::mm::address::operations::UsizeConvert::from_usize(
                    self.as_usize() | rhs.as_usize(),
                )
            }
        }
        impl core::ops::BitOrAssign<usize> for $type {
            fn bitor_assign(&mut self, rhs: usize) {
                *self =
                    $crate::mm::address::operations::UsizeConvert::from_usize(self.as_usize() | rhs)
            }
        }
        impl core::ops::BitOrAssign<Self> for $type {
            fn bitor_assign(&mut self, rhs: Self) {
                *self = $crate::mm::address::operations::UsizeConvert::from_usize(
                    self.as_usize() | rhs.as_usize(),
                )
            }
        }
        impl core::ops::BitXor<usize> for $type {
            type Output = Self;
            fn bitxor(self, rhs: usize) -> Self::Output {
                $crate::mm::address::operations::UsizeConvert::from_usize(self.as_usize() ^ rhs)
            }
        }
        impl core::ops::BitXor<Self> for $type {
            type Output = Self;
            fn bitxor(self, rhs: Self) -> Self::Output {
                $crate::mm::address::operations::UsizeConvert::from_usize(
                    self.as_usize() ^ rhs.as_usize(),
                )
            }
        }
        impl core::ops::BitXorAssign<usize> for $type {
            fn bitxor_assign(&mut self, rhs: usize) {
                *self =
                    $crate::mm::address::operations::UsizeConvert::from_usize(self.as_usize() ^ rhs)
            }
        }
        impl core::ops::BitXorAssign<Self> for $type {
            fn bitxor_assign(&mut self, rhs: Self) {
                *self = $crate::mm::address::operations::UsizeConvert::from_usize(
                    self.as_usize() ^ rhs.as_usize(),
                )
            }
        }
        impl core::ops::Shl<usize> for $type {
            type Output = Self;
            fn shl(self, rhs: usize) -> Self::Output {
                $crate::mm::address::operations::UsizeConvert::from_usize(self.as_usize() << rhs)
            }
        }
        impl core::ops::ShlAssign<usize> for $type {
            fn shl_assign(&mut self, rhs: usize) {
                *self = $crate::mm::address::operations::UsizeConvert::from_usize(
                    self.as_usize() << rhs,
                )
            }
        }
        impl core::ops::Shr<usize> for $type {
            type Output = Self;
            fn shr(self, rhs: usize) -> Self::Output {
                $crate::mm::address::operations::UsizeConvert::from_usize(self.as_usize() >> rhs)
            }
        }
        impl core::ops::ShrAssign<usize> for $type {
            fn shr_assign(&mut self, rhs: usize) {
                *self = $crate::mm::address::operations::UsizeConvert::from_usize(
                    self.as_usize() >> rhs,
                )
            }
        }
        impl $crate::mm::address::operations::CalcOps for $type {}
    };
}

// alignment operations
pub trait AlignOps: UsizeConvert {
    // check if the address is aligned to the given alignment
    fn is_aligned(self, alignment: usize) -> bool {
        debug_assert!(
            alignment.is_power_of_two(),
            "alignment must be a power of two"
        );
        let mask = alignment - 1;
        self.as_usize() & mask == 0
    }
    fn is_page_aligned(self) -> bool {
        self.is_aligned(PAGE_SIZE)
    }
    // align the address up to the given alignment
    fn align_up(self, alignment: usize) -> Self {
        debug_assert!(
            alignment.is_power_of_two(),
            "alignment must be a power of two"
        );
        let mask = alignment - 1;
        Self::from_usize((self.as_usize() + mask) & !mask)
    }
    // align the address down to the given alignment
    fn align_down(self, alignment: usize) -> Self {
        debug_assert!(
            alignment.is_power_of_two(),
            "alignment must be a power of two"
        );
        let mask = alignment - 1;
        Self::from_usize(self.as_usize() & !mask)
    }
    // align the address up to the page size
    fn align_up_to_page(self) -> Self {
        self.align_up(PAGE_SIZE)
    }
    // align the address down to the page size
    fn align_down_to_page(self) -> Self {
        self.align_down(PAGE_SIZE)
    }
}
