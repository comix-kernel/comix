//! 标准 I/O 文件实现
//!
//! 提供标准输入、输出、错误输出的文件接口，直接操作控制台，不依赖 Inode。

use crate::{
    uapi::time::timespec,
    vfs::{File, FileMode, FsError, InodeMetadata, InodeType},
};
use alloc::sync::Arc;

/// 标准输入文件
///
/// 从控制台读取输入，行缓冲模式。
pub struct StdinFile;

impl File for StdinFile {
    fn readable(&self) -> bool {
        true
    }

    fn writable(&self) -> bool {
        false
    }

    fn read(&self, buf: &mut [u8]) -> Result<usize, FsError> {
        use crate::arch::lib::sbi::console_getchar;

        let mut count = 0;
        for byte in buf.iter_mut() {
            let ch = console_getchar();
            if ch == 0 {
                break;
            }

            *byte = ch as u8;
            count += 1;

            if ch == b'\n' as usize {
                break;
            }
        }

        Ok(count)
    }

    fn write(&self, _buf: &[u8]) -> Result<usize, FsError> {
        Err(FsError::PermissionDenied)
    }

    fn metadata(&self) -> Result<InodeMetadata, FsError> {
        Ok(InodeMetadata {
            inode_no: 0,
            inode_type: InodeType::CharDevice,
            mode: FileMode::S_IFCHR | FileMode::S_IRUSR,
            uid: 0,
            gid: 0,
            size: 0,
            atime: timespec::now(),
            mtime: timespec::now(),
            ctime: timespec::now(),
            nlinks: 1,
            blocks: 0,
        })
    }

    // lseek 使用默认实现 (返回 NotSupported)
}

/// 标准输出文件
///
/// 输出到控制台，全缓冲模式。
pub struct StdoutFile;

impl File for StdoutFile {
    fn readable(&self) -> bool {
        false
    }

    fn writable(&self) -> bool {
        true
    }

    fn read(&self, _buf: &mut [u8]) -> Result<usize, FsError> {
        Err(FsError::PermissionDenied)
    }

    fn write(&self, buf: &[u8]) -> Result<usize, FsError> {
        use crate::arch::lib::sbi::console_putchar;
        for &byte in buf {
            console_putchar(byte as usize);
        }
        Ok(buf.len())
    }

    fn metadata(&self) -> Result<InodeMetadata, FsError> {
        Ok(InodeMetadata {
            inode_no: 1,
            inode_type: InodeType::CharDevice,
            mode: FileMode::S_IFCHR | FileMode::S_IWUSR,
            uid: 0,
            gid: 0,
            size: 0,
            atime: timespec::now(),
            mtime: timespec::now(),
            ctime: timespec::now(),
            nlinks: 1,
            blocks: 0,
        })
    }
}

/// 标准错误输出文件
///
/// 输出到控制台（与 stdout 相同），无缓冲模式。
pub struct StderrFile;

impl File for StderrFile {
    fn readable(&self) -> bool {
        false
    }

    fn writable(&self) -> bool {
        true
    }

    fn read(&self, _buf: &mut [u8]) -> Result<usize, FsError> {
        Err(FsError::PermissionDenied)
    }

    fn write(&self, buf: &[u8]) -> Result<usize, FsError> {
        use crate::arch::lib::sbi::console_putchar;
        for &byte in buf {
            console_putchar(byte as usize);
        }
        Ok(buf.len())
    }

    fn metadata(&self) -> Result<InodeMetadata, FsError> {
        Ok(InodeMetadata {
            inode_no: 2,
            inode_type: InodeType::CharDevice,
            mode: FileMode::S_IFCHR | FileMode::S_IWUSR,
            uid: 0,
            gid: 0,
            size: 0,
            atime: timespec::now(),
            mtime: timespec::now(),
            ctime: timespec::now(),
            nlinks: 1,
            blocks: 0,
        })
    }
}

/// 创建标准 I/O 文件对象 (替代 stdio.rs:211-237)
///
/// 返回: 三元组 (stdin, stdout, stderr)
pub fn create_stdio_files() -> (Arc<dyn File>, Arc<dyn File>, Arc<dyn File>) {
    (
        Arc::new(StdinFile),
        Arc::new(StdoutFile),
        Arc::new(StderrFile),
    )
}
