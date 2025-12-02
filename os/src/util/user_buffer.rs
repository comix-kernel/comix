//! 用户态缓冲区
//!
//! 一般来说，用户态程序通过一段位于用户地址的缓冲区与内核进行数据交换
//! 例如，系统调用接口通常传入指向用户缓冲区的指针和长度
//! 这个模块提供了对这类缓冲区的抽象和操作方法
//!
//! XXX: 目前的实现直接依赖于 RISC-V 特权级的 SUM 位来允许内核访问用户空间
//!      未来可能需要改进为通过页表映射等方式实现更通用的用户空间访问

use alloc::vec::Vec;
use core::ptr;

use crate::arch::constant::{USER_BASE, USER_TOP};
use crate::arch::trap::SumGuard;

/// 向用户空间写入数据
/// # 参数
/// - `user_ptr`: 指向用户空间的指针
/// - `value`: 要写入的数据
/// # Safety
/// 调用者必须确保 `user_ptr` 指向的内存是有效且可写的用户空间地址。
pub unsafe fn write_to_user<T>(user_ptr: *mut T, value: T) {
    let _guard = SumGuard::new();
    unsafe {
        ptr::write_volatile(user_ptr, value);
    }
}

/// 从用户空间读取数据
/// # 参数
/// - `user_ptr`: 指向用户空间的指针
/// # 返回值
/// - 读取到的数据
/// # Safety
/// 调用者必须确保 `user_ptr` 指向的内存是有效且可读的用户空间地址。
pub unsafe fn read_from_user<T: Copy>(user_ptr: *const T) -> T {
    let _guard = SumGuard::new();
    unsafe { ptr::read_volatile(user_ptr) }
}

/// 用户缓冲区结构体
pub struct UserBuffer {
    data: *mut u8,
    len: usize,
}

impl UserBuffer {
    /// 创建一个新的用户缓冲区
    /// # 参数：
    /// - `data`: 指向用户缓冲区的指针
    /// - `len`: 缓冲区的长度
    pub fn new(data: *mut u8, len: usize) -> Self {
        Self { data, len }
    }

    /// 从用户缓冲区向内核缓冲区复制数据
    /// # Safety
    /// - 调用方必须保证 `self.data .. self.data + self.len` 是用户空间中有效且已映射的可读内存；
    /// - 与目标内核缓冲区不重叠（此处目标是新分配的 Vec，天然满足）；
    /// - 若无法在此处静态保证有效性，应在更高一层先做页表/范围校验。
    pub unsafe fn copy_from_user(self) -> Vec<u8> {
        if self.len == 0 {
            return Vec::new();
        }
        let mut vec = Vec::with_capacity(self.len);
        unsafe {
            let _guard = SumGuard::new();
            vec.set_len(self.len);
            ptr::copy_nonoverlapping(self.data as *const u8, vec.as_mut_ptr(), self.len);
        }
        vec
    }

    /// 将内核缓冲区数据拷贝到用户缓冲区
    /// 超过用户缓冲区长度的部分将被截断
    /// # Safety
    /// - 调用方必须保证 `self.data .. self.data + self.len` 是用户空间中有效且已映射的可写内存；
    /// - 与源切片不重叠（此处源在内核内存，通常不与用户缓冲重叠）。
    pub unsafe fn copy_to_user(self, data: &[u8]) {
        if self.len == 0 || data.is_empty() {
            return;
        }
        let n = core::cmp::min(self.len, data.len());
        unsafe {
            let _guard = SumGuard::new();
            ptr::copy_nonoverlapping(data.as_ptr(), self.data, n);
        }
    }

    /// TODO: 运行时做一次“粗略”范围校验（不保证已映射，仅做地址区间与溢出检查）
    /// 建议在 syscall 层或结合 MemorySpace 做页表级校验。
    pub fn range_sane(&self) -> bool {
        unimplemented!();
        let start = self.data as usize;
        let end = start.checked_add(self.len).unwrap_or(usize::MAX);
        // start < USER_BASE && end <= USER_TOP;
        true
    }

    /// 返回用户缓冲区长度
    #[inline]
    pub fn len(&self) -> usize {
        self.len
    }

    /// 判断用户缓冲区是否为空
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }
}

/// 验证用户空间指针是否有效
///
/// 检查指针是否：
/// 1. 非空
/// 2. 指向用户空间地址范围 [USER_BASE, USER_TOP]
/// 3. 指针指向的内存区域不溢出用户空间
///
/// # 参数
/// * `ptr` - 要验证的用户空间指针
///
/// # 返回值
/// * `true` - 指针有效
/// * `false` - 指针无效
///
/// # 注意
/// 此函数仅进行地址范围检查，不验证内存是否已映射或可访问。
/// 实际访问内存前，仍需处理可能的页错误。
pub fn validate_user_ptr<T>(ptr: *const T) -> bool {
    if ptr.is_null() {
        return false;
    }

    let addr = ptr as usize;
    let size = core::mem::size_of::<T>();

    // 检查起始地址是否在用户空间范围内
    // USER_BASE 为 0，所以只需检查上界
    if addr > USER_TOP {
        return false;
    }

    // 检查是否会溢出用户空间
    if let Some(end_addr) = addr.checked_add(size) {
        if end_addr > USER_TOP + 1 {
            return false;
        }
    } else {
        // 地址加法溢出
        return false;
    }

    true
}

/// 验证可写的用户空间指针是否有效
///
/// 与 `validate_user_ptr` 功能相同，但用于可变指针。
///
/// # 参数
/// * `ptr` - 要验证的可写用户空间指针
///
/// # 返回值
/// * `true` - 指针有效
/// * `false` - 指针无效
#[inline]
pub fn validate_user_ptr_mut<T>(ptr: *mut T) -> bool {
    validate_user_ptr(ptr as *const T)
}
