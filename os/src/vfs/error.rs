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
    BadAddress,      // -EFAULT(14): 无效用户地址
    NameTooLong,     // -ENAMETOOLONG(36): 文件名过长

    // 文件系统相关
    ReadOnlyFs,            // -EROFS(30): 只读文件系统
    NoSpace,               // -ENOSPC(28): 设备空间不足
    IoError,               // -EIO(5): I/O 错误
    NoSuchDeviceOrAddress, // -ENXIO(6): 设备或地址不存在
    NoDevice,              // -ENODEV(19): 设备不存在
    NoMemory,              // -ENOMEM(12): 内存不足
    Busy,                  // -EBUSY(16): 设备或资源忙
    CrossDeviceLink,       // -EXDEV(18): 跨设备链接/重命名

    // 管道相关 (新增)
    BrokenPipe, // -EPIPE(32): 管道破裂 (读端已关闭)
    WouldBlock, // -EAGAIN(11): 非阻塞操作将阻塞

    // 网络相关
    DestinationAddressRequired, // -EDESTADDRREQ(89): 需要目标地址
    NotConnected,               // -ENOTCONN(107): 套接字未连接

    // 其他
    NotSupported,    // -ENOTSUP(95): 操作不支持
    NotTty,          // -ENOTTY(25): 非 TTY 设备或不支持该 ioctl
    NotSeekable,     // -ESPIPE(29): 不支持 seek
    TooManyLinks,    // -EMLINK(31): 硬链接过多
    TooManySymlinks, // -ELOOP(40): 符号链接层级过多
}

impl FsError {
    /// 转换为系统调用错误码（负数）
    pub fn to_errno(self) -> isize {
        use crate::uapi::errno::*;

        match self {
            FsError::NotFound => -ENOENT as isize,
            FsError::IoError => -EIO as isize,
            FsError::NoSuchDeviceOrAddress => -ENXIO as isize,
            FsError::BadFileDescriptor => -EBADF as isize,
            FsError::WouldBlock => -EAGAIN as isize,
            FsError::NoMemory => -ENOMEM as isize,
            FsError::PermissionDenied => -EACCES as isize,
            FsError::BadAddress => -EFAULT as isize,
            FsError::Busy => -EBUSY as isize,
            FsError::AlreadyExists => -EEXIST as isize,
            FsError::CrossDeviceLink => -EXDEV as isize,
            FsError::NoDevice => -ENODEV as isize,
            FsError::NotDirectory => -ENOTDIR as isize,
            FsError::IsDirectory => -EISDIR as isize,
            FsError::InvalidArgument => -EINVAL as isize,
            FsError::TooManyOpenFiles => -EMFILE as isize,
            FsError::NotTty => -ENOTTY as isize,
            FsError::NoSpace => -ENOSPC as isize,
            FsError::NotSeekable => -ESPIPE as isize,
            FsError::ReadOnlyFs => -EROFS as isize,
            FsError::TooManyLinks => -EMLINK as isize,
            FsError::BrokenPipe => -EPIPE as isize,
            FsError::NameTooLong => -ENAMETOOLONG as isize,
            FsError::DirectoryNotEmpty => -ENOTEMPTY as isize,
            FsError::TooManySymlinks => -ELOOP as isize,
            FsError::NotSupported => -EOPNOTSUPP as isize,
            FsError::DestinationAddressRequired => -EDESTADDRREQ as isize,
            FsError::NotConnected => -ENOTCONN as isize,
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{kassert, test_case};

    use super::*;

    test_case!(test_error_codes, {
        kassert!(FsError::NotFound.to_errno() == -crate::uapi::errno::ENOENT as isize);
        kassert!(FsError::PermissionDenied.to_errno() == -crate::uapi::errno::EACCES as isize);
        kassert!(FsError::AlreadyExists.to_errno() == -crate::uapi::errno::EEXIST as isize);
        kassert!(FsError::CrossDeviceLink.to_errno() == -crate::uapi::errno::EXDEV as isize);
        kassert!(FsError::BadAddress.to_errno() == -crate::uapi::errno::EFAULT as isize);
        kassert!(FsError::NotSeekable.to_errno() == -crate::uapi::errno::ESPIPE as isize);
        kassert!(FsError::NotTty.to_errno() == -crate::uapi::errno::ENOTTY as isize);
    });
}
