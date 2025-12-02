//! 向量 I/O 相关类型定义

/// iovec 结构体（对应 POSIX struct iovec）
///
/// 用于 readv/writev/preadv/pwritev 系统调用
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct IoVec {
    /// 缓冲区起始地址
    pub iov_base: *mut u8,

    /// 缓冲区长度
    pub iov_len: usize,
}

impl IoVec {
    /// 检查 iovec 是否有效
    pub fn is_valid(&self) -> bool {
        !self.iov_base.is_null() && self.iov_len > 0
    }
}
