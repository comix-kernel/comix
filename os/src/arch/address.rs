//! 类型安全的架构地址抽象
//!
//! 借鉴 moss-kernel 的地址类型体系设计：
//! - Sealed trait 防止外部实现（安全边界）
//! - 泛型 `Address<K, T>` 带 `MemKind` 和数据类型标记
//! - 编译期防止地址空间混用
//! - 安全访问控制：`PA::as_ptr()` unsafe, `VA::as_ptr()` safe, `UA` 不能直接转指针

use core::marker::PhantomData;

// ============================================================================
// Sealed trait — 外部无法实现 MemKind
// ============================================================================
mod sealed {
    pub trait Sealed {}
}

/// 内存地址种类标记 trait（sealed — 外部无法实现）
pub trait MemKind: sealed::Sealed + Ord + Clone + Copy + PartialEq + Eq + core::fmt::Debug {}

// ============================================================================
// 地址种类标记类型
// ============================================================================

/// 虚拟地址标记
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Virtual;
impl sealed::Sealed for Virtual {}
impl MemKind for Virtual {}

/// 物理地址标记
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Physical;
impl sealed::Sealed for Physical {}
impl MemKind for Physical {}

/// 用户空间地址标记
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct User;
impl sealed::Sealed for User {}
impl MemKind for User {}

// ============================================================================
// 泛型地址类型
// ============================================================================

/// 带地址种类 `K` 和数据类型 `T` 标记的地址
///
/// - `K`: 地址空间种类（Virtual / Physical / User）
/// - `T`: 指向的数据类型（`()` 表示无类型标记）
#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Address<K: MemKind, T> {
    inner: usize,
    _phantom: PhantomData<(K, T)>,
}

impl<K: MemKind, T> core::fmt::Debug for Address<K, T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Address")
            .field("inner", &format_args!("0x{:x}", self.inner))
            .finish()
    }
}

// ============================================================================
// 类型别名
// ============================================================================

/// 无类型标记的物理地址
pub type PA = Address<Physical, ()>;
/// 无类型标记的虚拟地址
pub type VA = Address<Virtual, ()>;
/// 无类型标记的用户空间地址
pub type UA = Address<User, ()>;

// ============================================================================
// Address 基本方法
// ============================================================================

impl<K: MemKind, T> Address<K, T> {
    /// 从 `usize` 创建地址
    pub const fn from_usize(addr: usize) -> Self {
        Self {
            inner: addr,
            _phantom: PhantomData,
        }
    }

    /// 获取地址的 `usize` 值
    pub const fn as_usize(&self) -> usize {
        self.inner
    }

    /// 检查地址是否为零
    pub fn is_null(self) -> bool {
        self.inner == 0
    }

    /// 返回零地址
    pub const fn null() -> Self {
        Self::from_usize(0)
    }

    /// 获取页内偏移
    pub fn page_offset(self) -> usize {
        self.inner & (crate::config::PAGE_SIZE - 1)
    }

    /// 检查地址是否页对齐
    pub fn is_page_aligned(self) -> bool {
        self.page_offset() == 0
    }

    /// 向上对齐到页边界
    pub fn page_aligned(self) -> Self {
        let page_size = crate::config::PAGE_SIZE;
        Self::from_usize((self.inner + page_size - 1) & !(page_size - 1))
    }

    /// 向下对齐到页边界
    pub fn align_down_to_page(self) -> Self {
        let page_size = crate::config::PAGE_SIZE;
        Self::from_usize(self.inner & !(page_size - 1))
    }

    /// 向上对齐到指定对齐值
    pub fn align_up(self, align: usize) -> Self {
        Self::from_usize((self.inner + align - 1) & !(align - 1))
    }

    /// 增加字节偏移
    pub fn add_bytes(self, offset: usize) -> Self {
        Self::from_usize(self.inner + offset)
    }

    /// 减去字节偏移
    pub fn sub_bytes(self, offset: usize) -> Self {
        Self::from_usize(self.inner - offset)
    }

    /// 增加页数
    pub fn add_pages(self, count: usize) -> Self {
        self.add_bytes(count * crate::config::PAGE_SIZE)
    }

    /// 计算与另一地址的差值
    pub fn diff(self, other: Self) -> isize {
        self.inner as isize - other.inner as isize
    }
}

// ============================================================================
// 物理地址 (PA) 特有方法
// ============================================================================

impl<T> Address<Physical, T> {
    /// 将物理地址转换为裸指针（只读）
    ///
    /// # Safety
    ///
    /// 裸物理地址访问需要显式承诺：调用者必须确保物理地址有效且已映射。
    pub unsafe fn as_ptr<U>(&self) -> *const U {
        self.inner as *const U
    }

    /// 将物理地址转换为可变裸指针
    ///
    /// # Safety
    ///
    /// 裸物理地址访问需要显式承诺：调用者必须确保物理地址有效且已映射，
    /// 并且没有其他活跃引用指向同一内存。
    pub unsafe fn as_mut_ptr<U>(&mut self) -> *mut U {
        self.inner as *mut U
    }
}

// ============================================================================
// 虚拟地址 (VA) 特有方法
// ============================================================================

impl<T> Address<Virtual, T> {
    /// 将虚拟地址转换为裸指针（只读）
    ///
    /// 虚拟地址已通过 MMU 映射，因此此操作不是 unsafe。
    pub fn as_ptr<U>(&self) -> *const U {
        self.inner as *const U
    }

    /// 将虚拟地址转换为可变裸指针
    pub fn as_mut_ptr<U>(&mut self) -> *mut U {
        self.inner as *mut U
    }

    /// 将虚拟地址转换为不可变引用
    ///
    /// # Safety
    ///
    /// 调用者必须确保地址指向的内存已初始化且未被其他可变引用借用。
    pub unsafe fn as_ref<'a, U>(&self) -> &'a U {
        unsafe { &*(self.inner as *const U) }
    }

    /// 将虚拟地址转换为可变引用
    ///
    /// # Safety
    ///
    /// 调用者必须确保地址指向的内存已初始化且无其他活跃引用。
    pub unsafe fn as_mut<'a, U>(&mut self) -> &'a mut U {
        unsafe { &mut *(self.inner as *mut U) }
    }
}

impl Address<Virtual, ()> {
    /// 从一个不可变引用创建虚拟地址。
    pub fn from_ref<T>(r: &T) -> Self {
        Self::from_ptr(r as *const T)
    }

    /// 从一个常量指针创建虚拟地址。
    pub fn from_ptr<T>(p: *const T) -> Self {
        Self::from_usize(p as usize)
    }
}

// ============================================================================
// 用户地址 (UA) — 不能直接转指针，必须通过安全机制访问
// ============================================================================

impl<T> Address<User, T> {
    /// 用户地址不能直接转换为裸指针。
    /// 必须通过 `copy_from_user`/`copy_to_user` 等安全机制访问。
    #[deprecated(note = "UA 不能直接转指针，请使用 copy_from_user/copy_to_user")]
    pub fn as_ptr(&self) -> *const T {
        panic!("UA::as_ptr() is forbidden; use copy_from_user/copy_to_user")
    }
}

// ============================================================================
// unsafe impl Send/Sync
// ============================================================================

unsafe impl<K: MemKind, T> Send for Address<K, T> {}
unsafe impl<K: MemKind, T> Sync for Address<K, T> {}
