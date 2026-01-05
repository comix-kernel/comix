//! VFS 错误类型
//!
//! 定义了与 POSIX 兼容的文件系统错误码，可通过 [`FsError::to_errno()`] 转换为系统调用错误码。

/// VFS 错误类型
///
/// 各错误码对应标准 POSIX errno 值。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FsError {
    // 文件/目录相关
    NotFound,          // -ENOENT(2): 文件不存在
    AlreadyExists,     // -EEXIST(17): 文件已存在
    NotDirectory,      // -ENOTDIR(20): 不是目录
    IsDirectory,       // -EISDIR(21): 是目录
    DirectoryNotEmpty, // -ENOTEMPTY(39): 目录非空

    // 权限相关
    PermissionDenied, // -EACCES(13): 权限被拒绝

    // 文件描述符相关
    BadFileDescriptor, // -EBADF(9): 无效的文件描述符
    TooManyOpenFiles,  // -EMFILE(24): 打开的文件过多

    // 参数相关
    InvalidArgument, // -EINVAL(22): 无效参数
    NameTooLong,     // -ENAMETOOLONG(36): 文件名过长

    // 文件系统相关
    ReadOnlyFs, // -EROFS(30): 只读文件系统
    NoSpace,    // -ENOSPC(28): 设备空间不足
    IoError,    // -EIO(5): I/O 错误
    NoDevice,   // -ENODEV(19): 设备不存在

    // 管道相关 (新增)
    BrokenPipe, // -EPIPE(32): 管道破裂 (读端已关闭)
    WouldBlock, // -EAGAIN(11): 非阻塞操作将阻塞

    // 网络相关
    NotConnected, // -ENOTCONN(107): 套接字未连接

    // 其他
    NotSupported, // -ENOTSUP(95): 操作不支持
    TooManyLinks, // -EMLINK(31): 硬链接过多
    TooManySymlinks, // -ELOOP(40): 符号链接层级过多
}

impl FsError {
    /// 转换为系统调用错误码（负数）
    pub fn to_errno(&self) -> isize {
        match self {
            FsError::NotFound => -2,
            FsError::IoError => -5,
            FsError::BadFileDescriptor => -9,
            FsError::WouldBlock => -11,
            FsError::PermissionDenied => -13,
            FsError::AlreadyExists => -17,
            FsError::NoDevice => -19,
            FsError::NotDirectory => -20,
            FsError::IsDirectory => -21,
            FsError::InvalidArgument => -22,
            FsError::TooManyOpenFiles => -24,
            FsError::NoSpace => -28,
            FsError::ReadOnlyFs => -30,
            FsError::TooManyLinks => -31,
            FsError::TooManySymlinks => -40,
            FsError::BrokenPipe => -32,
            FsError::NameTooLong => -36,
            FsError::DirectoryNotEmpty => -39,
            FsError::NotSupported => -95,
            FsError::NotConnected => -107,
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{kassert, test_case};

    use super::*;

    test_case!(test_error_codes, {
        kassert!(FsError::NotFound.to_errno() == -2);
        kassert!(FsError::PermissionDenied.to_errno() == -13);
        kassert!(FsError::AlreadyExists.to_errno() == -17);
    });
}
