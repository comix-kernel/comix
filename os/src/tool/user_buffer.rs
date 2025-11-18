//! 用户态缓冲区
//!
//! 一般来说，用户态程序通过一段位于用户地址的缓冲区与内核进行数据交换
//! 例如，系统调用接口通常传入指向用户缓冲区的指针和长度
//! 这个模块提供了对这类缓冲区的抽象和操作方法

use alloc::vec::Vec;
use core::ptr;
use riscv::register::sstatus;

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
            sstatus::set_sum();
            vec.set_len(self.len);
            ptr::copy_nonoverlapping(self.data as *const u8, vec.as_mut_ptr(), self.len);
            sstatus::clear_sum();
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
            sstatus::set_sum();
            ptr::copy_nonoverlapping(data.as_ptr(), self.data, n);
            sstatus::clear_sum();
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
