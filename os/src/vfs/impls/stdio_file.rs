//! 标准 I/O 文件实现
//!
//! 提供标准输入、输出、错误输出的文件接口，直接操作控制台，不依赖 Inode。

use crate::{
    sync::SpinLock,
    uapi::ioctl::Termios,
    uapi::time::TimeSpec,
    vfs::{File, FileMode, FsError, InodeMetadata, InodeType},
};
use alloc::sync::Arc;

/// 全局终端设置（所有标准I/O文件共享）
static STDIO_TERMIOS: SpinLock<Termios> = SpinLock::new(Termios {
    c_iflag: 0x0100,
    c_oflag: 0x0001 | 0x0004,
    c_cflag: 0x0030 | 0x0080,
    c_lflag: 0x0001 | 0x0002 | 0x0008 | 0x0010,
    c_line: 0,
    c_cc: [3, 28, 127, 21, 4, 0, 1, 0, 17, 19, 26, 0, 18, 15, 23, 22, 0, 0, 0],
    c_ispeed: 0x0000000f,
    c_ospeed: 0x0000000f,
});

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
            atime: TimeSpec::now(),
            mtime: TimeSpec::now(),
            ctime: TimeSpec::now(),
            nlinks: 1,
            blocks: 0,
            rdev: 0,
        })
    }

    fn ioctl(&self, request: u32, arg: usize) -> Result<isize, FsError> {
        stdio_ioctl(request, arg)
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
            atime: TimeSpec::now(),
            mtime: TimeSpec::now(),
            ctime: TimeSpec::now(),
            nlinks: 1,
            blocks: 0,
            rdev: 0,
        })
    }

    fn ioctl(&self, request: u32, arg: usize) -> Result<isize, FsError> {
        stdio_ioctl(request, arg)
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
            atime: TimeSpec::now(),
            mtime: TimeSpec::now(),
            ctime: TimeSpec::now(),
            nlinks: 1,
            blocks: 0,
            rdev: 0,
        })
    }

    fn ioctl(&self, request: u32, arg: usize) -> Result<isize, FsError> {
        stdio_ioctl(request, arg)
    }
}

/// 通用的 stdio ioctl 实现
fn stdio_ioctl(request: u32, arg: usize) -> Result<isize, FsError> {
    use crate::uapi::errno::{EINVAL, ENOTTY};
    use crate::uapi::ioctl::*;
    use riscv::register::sstatus;

    match request {
        TCGETS => {
            if arg == 0 {
                return Ok(-EINVAL as isize);
            }

            unsafe {
                sstatus::set_sum();
                let termios_ptr = arg as *mut Termios;
                if termios_ptr.is_null() {
                    sstatus::clear_sum();
                    return Ok(-EINVAL as isize);
                }

                // 清零结构体（包括 padding），避免泄露内核栈数据
                core::ptr::write_bytes(termios_ptr, 0, 1);

                // 返回保存的 termios 设置
                let termios = *STDIO_TERMIOS.lock();
                core::ptr::write_volatile(termios_ptr, termios);
                sstatus::clear_sum();

                // 调试：打印返回的termios内容
                use crate::earlyprintln;
                earlyprintln!("TCGETS: returning termios: iflag={:#x}, oflag={:#x}, cflag={:#x}, lflag={:#x}, ispeed={:#x}, ospeed={:#x}",
                    termios.c_iflag, termios.c_oflag, termios.c_cflag, termios.c_lflag, termios.c_ispeed, termios.c_ospeed);
            }
            Ok(0)
        }

        TCSETS | TCSETSW | TCSETSF => {
            if arg == 0 {
                return Ok(-EINVAL as isize);
            }

            unsafe {
                sstatus::set_sum();
                let termios_ptr = arg as *const Termios;
                if termios_ptr.is_null() {
                    sstatus::clear_sum();
                    return Ok(-EINVAL as isize);
                }

                // 读取新的 termios 设置并保存
                let new_termios = core::ptr::read_volatile(termios_ptr);

                // 调试：打印接收到的termios内容
                use crate::earlyprintln;
                earlyprintln!("TCSETS: received termios: iflag={:#x}, oflag={:#x}, cflag={:#x}, lflag={:#x}, ispeed={:#x}, ospeed={:#x}",
                    new_termios.c_iflag, new_termios.c_oflag, new_termios.c_cflag, new_termios.c_lflag, new_termios.c_ispeed, new_termios.c_ospeed);

                *STDIO_TERMIOS.lock() = new_termios;
                sstatus::clear_sum();
            }
            Ok(0)
        }

        // 其他 ioctl 命令不支持
        _ => Ok(-ENOTTY as isize),
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
