//! 地址抽象模块
//!
//! 此模块定义了表示内存地址的核心 Trait ([Address])，以及具体的
//! 地址类型 ([Paddr] 物理地址, [Vaddr] 虚拟地址)，并提供了地址算术、
//! 对齐操作以及地址范围 ([AddressRange]) 的支持。

use crate::mm::address::operations::{AlignOps, CalcOps, UsizeConvert};
use core::mem::size_of;
use core::ops::Range;

/// [Address] Trait
/// ---------------------
/// 表示一个内存地址的 Trait。所有地址类型必须实现此 Trait。
/// 它组合了算术运算 ([CalcOps])、对齐操作 ([AlignOps]) 和 usize 转换 ([UsizeConvert])。
pub trait Address:
    CalcOps + AlignOps + UsizeConvert + Copy + Clone + PartialEq + PartialOrd + Eq + Ord
{
    /// 检查地址是否为空 (即零)。
    fn is_null(self) -> bool {
        self.as_usize() == 0
    }

    /// 返回一个空地址 (零地址)。
    fn null() -> Self {
        Self::from_usize(0)
    }

    /// 获取地址在当前页内的偏移量。
    fn page_offset(self) -> usize {
        // 使用位掩码 (PAGE_SIZE - 1) 快速计算页内偏移
        self.as_usize() & (crate::config::PAGE_SIZE - 1)
    }

    /// 计算两个地址之间的字节差值。
    fn addr_diff(self, other: Self) -> isize {
        self.as_usize() as isize - other.as_usize() as isize
    }

    /// 将地址增加 `T` 类型的大小。
    ///
    /// # 泛型
    /// * `T`: 要增加其大小的类型。
    fn add<T>(self) -> Self {
        self.add_by(size_of::<T>())
    }

    /// 将地址增加 `n` 个 `T` 类型元素的大小。
    ///
    /// # 参数
    /// * `n`: 元素个数。
    fn add_n<T>(self, n: usize) -> Self {
        self.add_by(size_of::<T>() * n)
    }

    /// 将地址增加指定的字节偏移量。
    fn add_by(self, offset: usize) -> Self {
        Self::from_usize(self.as_usize() + offset)
    }

    /// 将地址减去 `Self` 类型的大小 (通常是 `size_of::<usize>`)。
    fn sub(self) -> Self {
        self.sub_by(size_of::<Self>())
    }

    /// 将地址减去 `n` 个 `Self` 类型元素的大小。
    fn sub_n(self, n: usize) -> Self {
        self.sub_by(size_of::<Self>() * n)
    }

    /// 将地址减去指定的字节偏移量。
    fn sub_by(self, offset: usize) -> Self {
        Self::from_usize(self.as_usize() - offset)
    }

    /// 将地址增加 `Self` 类型的大小 (原地修改)。
    fn step(&mut self) {
        self.step_by(size_of::<Self>())
    }

    /// 将地址增加 `n` 个 `Self` 类型元素的大小 (原地修改)。
    fn step_n(&mut self, n: usize) {
        self.step_by(size_of::<Self>() * n)
    }

    /// 将地址减去 `Self` 类型的大小 (原地修改)。
    fn step_back(&mut self) {
        self.step_back_by(size_of::<Self>())
    }

    /// 将地址减去 `n` 个 `Self` 类型元素的大小 (原地修改)。
    fn step_back_n(&mut self, n: usize) {
        self.step_back_by(size_of::<Self>() * n)
    }

    /// 将地址增加给定的字节偏移量 (原地修改)。
    fn step_by(&mut self, offset: usize) {
        *self = self.add_by(offset);
    }

    /// 将地址减去给定的字节偏移量 (原地修改)。
    fn step_back_by(&mut self, offset: usize) {
        *self = self.sub_by(offset);
    }
}

/// `impl_address!` 宏
/// ---------------------
/// 快速为地址类型实现所有必需的 Trait: [UsizeConvert], [CalcOps], [AlignOps], [Address]。
///
/// 注意：这里使用 `transmute` 在地址类型 (例如 `Paddr(*const ())`) 和 `usize` 之间进行
/// 零开销转换，这是操作地址类型时的标准做法。
#[macro_export]
macro_rules! impl_address {
    ($type:ty) => {
        impl $crate::mm::address::operations::UsizeConvert for $type {
            /// 将地址类型转换为其原始的 `usize` 值。
            fn as_usize(&self) -> usize {
                // SAFETY: 地址类型 (Paddr/Vaddr) 是 transparent 的，并且保证和 usize 大小相同。
                unsafe { core::mem::transmute::<Self, usize>(*self) }
            }

            /// 从 `usize` 值创建地址类型。
            fn from_usize(value: usize) -> Self {
                // SAFETY: 这是一个零开销的转换，将原始值转换为地址类型。
                unsafe { core::mem::transmute::<usize, Self>(value) }
            }
        }

        $crate::impl_calc_ops!($type);
        impl $crate::mm::address::operations::AlignOps for $type {}

        impl $crate::mm::address::address::Address for $type {}

        // 地址类型通常需要在多线程环境下传递和共享
        unsafe impl Sync for $type {}
        unsafe impl Send for $type {}
    };
}

/// [ConvertablePaddr] Trait
/// ---------------------
/// 物理地址转换为虚拟地址的能力。
pub trait ConvertablePaddr {
    /// 检查地址是否是有效的物理地址。
    fn is_valid_paddr(&self) -> bool;
    /// 将物理地址转换为虚拟地址。
    fn to_vaddr(&self) -> Vaddr;
}

/// [Paddr] (Physical Address)
/// ---------------------
/// 物理内存地址，对应于内存芯片上的实际位置。
#[repr(transparent)]
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub struct Paddr(pub *const ());
impl_address!(Paddr);

impl ConvertablePaddr for Paddr {
    fn is_valid_paddr(&self) -> bool {
        // 注意: 实际实现依赖于具体的架构函数 `vaddr_to_paddr`，
        // 这里的逻辑通常需要检查地址是否在物理内存范围内。
        self.as_usize() == unsafe { crate::arch::mm::vaddr_to_paddr(self.as_usize()) }
    }

    fn to_vaddr(&self) -> Vaddr {
        // 依赖于架构特定的映射函数 (例如：线性映射或固定偏移)
        Vaddr::from_usize(crate::arch::mm::paddr_to_vaddr(self.as_usize()))
    }
}

/// [ConvertableVaddr] Trait
/// ---------------------
/// 虚拟地址转换为物理地址的能力。
pub trait ConvertableVaddr {
    /// 检查地址是否是有效的虚拟地址。
    fn is_valid_vaddr(&self) -> bool;
    /// 将虚拟地址转换为物理地址。
    fn to_paddr(&self) -> Paddr;
}

/// [Vaddr] (Virtual Address)
/// ---------------------
/// 虚拟内存地址，对应于进程或内核的页表映射空间中的位置。
#[repr(transparent)]
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub struct Vaddr(pub *const ());
impl_address!(Vaddr);

impl ConvertableVaddr for Vaddr {
    fn is_valid_vaddr(&self) -> bool {
        // 注意: 实际实现通常涉及查询页表来确定映射关系。
        self.as_usize() == crate::arch::mm::paddr_to_vaddr(self.as_usize())
    }

    fn to_paddr(&self) -> Paddr {
        // 依赖于架构特定的反向映射函数 (查询页表或固定偏移)
        Paddr::from_usize(unsafe { crate::arch::mm::vaddr_to_paddr(self.as_usize()) })
    }
}

impl Vaddr {
    /// 从一个不可变引用创建虚拟地址。
    pub fn from_ref<T>(r: &T) -> Self {
        Self::from_ptr(r as *const T)
    }

    /// 从一个常量指针创建虚拟地址。
    pub fn from_ptr<T>(p: *const T) -> Self {
        Self::from_usize(p as usize)
    }

    /// 将虚拟地址转换为一个不可变引用。
    ///
    /// # Safety (不安全函数)
    /// 调用者必须确保：
    /// 1. 地址指向的内存是 **有效** 的且 **已初始化** 的 `T` 类型数据。
    /// 2. 内存在引用的生命周期内不会被修改 (即引用是不可变的)。
    /// 3. 地址已经正确映射，且对齐满足 `T` 的要求。
    pub unsafe fn as_ref<T>(&self) -> &T {
        unsafe { &*(self.as_usize() as *const T) }
    }

    /// 将虚拟地址转换为一个可变引用。
    ///
    /// # Safety (不安全函数)
    /// 调用者必须确保：
    /// 1. 地址指向的内存是 **有效** 的且 **已初始化** 的 `T` 类型数据。
    /// 2. 内存是 **独占** 的 (没有其他活跃的引用或指针指向它)。
    /// 3. 地址已经正确映射，且对齐满足 `T` 的要求。
    pub unsafe fn as_mut<T>(&mut self) -> &mut T {
        unsafe { &mut *(self.as_usize() as *mut T) }
    }

    /// 将虚拟地址转换为一个常量指针。
    pub fn as_ptr<T>(&self) -> *const T {
        self.as_usize() as *const T
    }

    /// 将虚拟地址转换为一个可变指针。
    ///
    /// # Safety (不安全函数)
    /// 调用者必须确保地址在被解引用时是有效的。
    pub unsafe fn as_mut_ptr<T>(&mut self) -> *mut T {
        self.as_usize() as *mut T
    }
}

/// [AddressRange]
/// ---------------------
/// 泛型地址范围结构，表示一个半开半闭的区间 `[start, end)`。
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AddressRange<T>
where
    T: Address,
{
    /// 范围的起始地址 (包含)。
    start: T,
    /// 范围的结束地址 (不包含)。
    end: T,
}

impl<T> AddressRange<T>
where
    T: Address,
{
    /// 创建一个新的地址范围。
    pub fn new(start: T, end: T) -> Self {
        Self { start, end }
    }

    /// 从 Rust 标准库的 `Range<T>` 创建一个地址范围。
    pub fn from_range(range: Range<T>) -> Self {
        Self {
            start: range.start,
            end: range.end,
        }
    }

    /// 从起始地址和长度 (字节数) 创建一个地址范围。
    pub fn from_start_len(start: T, len: usize) -> Self {
        Self {
            start,
            end: T::from_usize(start.as_usize() + len),
        }
    }

    /// 从地址切片中创建地址范围 (使用第一个和最后一个地址)。
    ///
    /// # 返回
    /// 如果切片长度小于 2，返回 `None`。
    pub fn from_slices(slices: &[T]) -> Option<Self> {
        if slices.len() < 2 {
            return None;
        }
        // 注意: 这里假设切片中的地址是连续的，并计算到最后一个元素的结束地址。
        //      实际中可能需要更复杂的逻辑来确定范围。
        Some(Self {
            start: slices[0],
            end: slices[slices.len() - 1],
        })
    }

    /// 获取起始地址。
    pub fn start(&self) -> T {
        self.start
    }

    /// 获取结束地址 (不包含)。
    pub fn end(&self) -> T {
        self.end
    }

    /// 获取范围的字节长度。
    pub fn len(&self) -> usize {
        debug_assert!(self.end.as_usize() >= self.start.as_usize());
        self.end.as_usize() - self.start.as_usize()
    }

    /// 检查范围是否为空 (即 start == end)。
    pub fn empty(&self) -> bool {
        self.start == self.end
    }

    /// 检查范围是否包含给定的地址。
    pub fn contains(&self, addr: T) -> bool {
        addr >= self.start && addr < self.end
    }

    /// 检查范围是否包含另一个范围。
    pub fn contains_range(&self, other: &Self) -> bool {
        other.start >= self.start && other.end <= self.end
    }

    /// 检查此范围是否包含在另一个范围中。
    pub fn contains_in(&self, other: &Self) -> bool {
        self.start >= other.start && self.end <= other.end
    }

    /// 检查两个范围是否相交 (有共同的字节)。
    pub fn intersects(&self, other: &Self) -> bool {
        self.start < other.end && other.start < self.end
    }

    /// 检查两个范围是否邻接 (一个的结束地址等于另一个的起始地址)。
    pub fn adjacent(&self, other: &Self) -> bool {
        self.end == other.start || other.end == self.start
    }

    /// 获取两个范围的交集。
    ///
    /// # 返回
    /// 如果不相交，返回 `None`。
    pub fn intersection(&self, other: &Self) -> Option<Self> {
        if !self.intersects(other) {
            return None;
        }
        let start = core::cmp::max(self.start, other.start);
        let end = core::cmp::min(self.end, other.end);
        Some(Self { start, end })
    }

    /// 获取两个范围的并集。
    ///
    /// # 返回
    /// 如果既不相交也不邻接，返回 `None`。
    pub fn union(&self, other: &Self) -> Option<Self> {
        if !self.intersects(other) && !self.adjacent(other) {
            return None;
        }
        let start = core::cmp::min(self.start, other.start);
        let end = core::cmp::max(self.end, other.end);
        Some(Self { start, end })
    }

    /// 获取范围的迭代器。
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

/// [AddressRangeIterator]
/// ---------------------
/// 地址范围的迭代器，每次步进 `size_of::<T>()` 字节。
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
        self.current.step(); // 步进 Self 类型的大小
        Some(addr)
    }
}

/// 物理地址范围的类型别名
pub type PaddrRange = AddressRange<Paddr>;

/// 虚拟地址范围的类型别名
pub type VaddrRange = AddressRange<Vaddr>;

#[cfg(test)]
mod address_basic_tests {
    use super::*;
    // 假设 arch 模块提供了 paddr_to_vaddr 的桩实现
    use crate::arch::mm::paddr_to_vaddr;
    use crate::{kassert, test_case};

    // 1.1 Paddr/Vaddr 创建和转换测试
    test_case!(test_address_roundtrip, {
        let test_values = [0x0, 0x1000, 0x8000_0000, 0x8000_1234];

        for &val in &test_values {
            let paddr = Paddr::from_usize(val);
            kassert!(paddr.as_usize() == val);

            let vaddr = Vaddr::from_usize(val);
            kassert!(vaddr.as_usize() == val);
        }
    });

    // 1.2 空地址测试
    test_case!(test_null_address, {
        let paddr = Paddr::null();
        kassert!(paddr.is_null());
        kassert!(paddr.as_usize() == 0);
    });

    // 1.3 页内偏移测试 (假设 PAGE_SIZE = 0x1000)
    test_case!(test_page_offset, {
        let cases = [(0x8000_0000, 0), (0x8000_0123, 0x123), (0x8000_0FFF, 0xFFF)];
        for &(addr, expected) in &cases {
            // Vaddr 和 Paddr 的行为应该相同
            kassert!(Paddr::from_usize(addr).page_offset() == expected);
            kassert!(Vaddr::from_usize(addr).page_offset() == expected);
        }
    });

    // 1.4 Paddr ↔ Vaddr 转换测试 (依赖于 arch/mm 桩实现)
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

    // 1.5 地址比较测试
    test_case!(test_address_comparison, {
        let a1 = Paddr::from_usize(0x8000_0000);
        let a2 = Paddr::from_usize(0x8000_0000);
        let a3 = Paddr::from_usize(0x8000_1000);

        kassert!(a1 == a2);
        kassert!(a1 < a3);
        kassert!(a3 > a1);
    });

    // 1.6 地址算术和步进测试
    test_case!(test_address_arithmetic, {
        let start = Paddr::from_usize(0x1000);

        // add_by
        kassert!(start.add_by(0x123).as_usize() == 0x1123);

        // add<u32> (size_of::<u32>() == 4)
        kassert!(start.add::<u32>().as_usize() == 0x1004);

        // add_n<u16> (size_of::<u16>() * 3 == 6)
        kassert!(start.add_n::<u16>(3).as_usize() == 0x1006);

        // step (Paddr::size_of() == size_of::<usize>())
        let mut p = start;
        p.step();
        kassert!(p.as_usize() == start.as_usize() + size_of::<Paddr>());

        // step_back_by
        let mut p = Paddr::from_usize(0x2000);
        p.step_back_by(0x10);
        kassert!(p.as_usize() == 0x1FF0);
    });
}
