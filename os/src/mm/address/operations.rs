//! Address Operations Module
//!
//! 此模块定义了用于自定义地址类型（如 Paddr 和 Vaddr）的数学、位操作和对齐 Trait。
//! 目标是使强类型地址在使用时具备与 `usize` 相同的运算能力，同时保持类型安全。

use crate::config::PAGE_SIZE;
use core::ops::{
    Add, AddAssign, BitAnd, BitAndAssign, BitOr, BitOrAssign, BitXor, BitXorAssign, Shl, ShlAssign,
    Shr, ShrAssign, Sub, SubAssign,
};

/// [UsizeConvert] Trait
/// ---------------------
/// 允许类型与 usize 之间互相转换。
/// 任何自定义的地址或页码类型 (例如 Paddr, Vaddr) 必须实现此 Trait，
/// 以便进行底层数值操作。
pub trait UsizeConvert: Copy + Clone + PartialEq + PartialOrd + Eq + Ord {
    /// 将类型转换为 usize。
    fn as_usize(&self) -> usize;
    /// 将 usize 转换为类型。
    fn from_usize(value: usize) -> Self;
}

/// [CalcOps] Trait
/// ---------------------
/// 定义了地址或页码类型所需的所有算术和位操作。
/// 统一了类型自身以及类型与 usize 之间的加、减、位运算和移位操作。
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

/// `impl_calc_ops!` 宏
/// ---------------------
/// 快速为给定类型实现所有 [CalcOps] 所需的 Trait 方法。
/// 这些实现通过先转换为 usize 进行计算，然后将结果转回类型来完成。
///
/// # 使用示例
/// ```ignore
/// // 在 MyAddr 上实现 UsizeConvert
/// impl_calc_ops!(MyAddr);
/// ```
#[macro_export]
macro_rules! impl_calc_ops {
    ($type:ty) => {
        // --- 加法实现 ---
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
        // --- 减法实现 ---
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
        // --- 位与实现 ---
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
        // --- 位或实现 ---
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
        // --- 位异或实现 ---
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
        // --- 移位实现 ---
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
        // 标记该类型已实现 CalcOps
        impl $crate::mm::address::operations::CalcOps for $type {}
    };
}

/// [AlignOps] Trait
/// ---------------------
/// 定义了地址对齐操作，例如检查对齐、向上对齐和向下对齐。
///
/// 注意: 所有对齐操作都要求 `alignment` 是 2 的幂。
#[allow(dead_code)]
pub trait AlignOps: UsizeConvert {
    /// 检查地址是否已对齐到给定的对齐边界。
    ///
    /// # 参数
    /// * `alignment`: 对齐边界（必须是 2 的幂）。
    fn is_aligned(self, alignment: usize) -> bool {
        debug_assert!(
            alignment.is_power_of_two(),
            "alignment must be a power of two"
        );
        let mask = alignment - 1;
        // 检查地址的低位是否全为 0
        self.as_usize() & mask == 0
    }
    /// 检查地址是否已页对齐（对齐到 `PAGE_SIZE`）。
    fn is_page_aligned(self) -> bool {
        self.is_aligned(PAGE_SIZE)
    }
    /// 将地址向上对齐到给定的对齐边界。
    ///
    /// # 参数
    /// * `alignment`: 对齐边界（必须是 2 的幂）。
    fn align_up(self, alignment: usize) -> Self {
        debug_assert!(
            alignment.is_power_of_two(),
            "alignment must be a power of two"
        );
        let mask = alignment - 1;
        // 核心逻辑: (addr + mask) & !mask
        Self::from_usize((self.as_usize() + mask) & !mask)
    }
    /// 将地址向下对齐到给定的对齐边界。
    ///
    /// # 参数
    /// * `alignment`: 对齐边界（必须是 2 的幂）。
    fn align_down(self, alignment: usize) -> Self {
        debug_assert!(
            alignment.is_power_of_two(),
            "alignment must be a power of two"
        );
        let mask = alignment - 1;
        // 核心逻辑: addr & !mask
        Self::from_usize(self.as_usize() & !mask)
    }
    /// 将地址向上对齐到页大小（`PAGE_SIZE`）。
    fn align_up_to_page(self) -> Self {
        self.align_up(PAGE_SIZE)
    }
    /// 将地址向下对齐到页大小（`PAGE_SIZE`）。
    fn align_down_to_page(self) -> Self {
        self.align_down(PAGE_SIZE)
    }
}
