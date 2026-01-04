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
static STDIO_TERMIOS: SpinLock<Termios> = SpinLock::new(Termios::DEFAULT);

/// 全局窗口大小（所有标准I/O文件共享）
static STDIO_WINSIZE: SpinLock<crate::uapi::ioctl::WinSize> =
    SpinLock::new(crate::uapi::ioctl::WinSize {
        ws_row: 24,
        ws_col: 80,
        ws_xpixel: 0,
        ws_ypixel: 0,
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
        use crate::console::{getchar as console_getchar, putchar as console_putchar};

        // Minimal termios handling for stdio-backed console
        // Align behavior with CharDeviceFile so CR/NL, echo, and canonical mode work as expected
        const ICRNL: u32 = 0x0100; // map CR->NL on input
        const INLCR: u32 = 0x0040; // map NL->CR on input
        const IGNCR: u32 = 0x0080; // ignore CR on input
        const ICANON: u32 = 0x0002; // canonical input
        const ECHO: u32 = 0x0008; // echo input

        let term = *STDIO_TERMIOS.lock();
        let canonical = (term.c_lflag & ICANON) != 0;
        let do_echo = (term.c_lflag & ECHO) != 0;

        let mut count = 0usize;
        // Lightweight, rate-limited diagnostics to confirm console input path
        use core::sync::atomic::{AtomicUsize, Ordering};
        static READ_LOG_COUNT: AtomicUsize = AtomicUsize::new(0);
        let log_once = READ_LOG_COUNT.fetch_add(1, Ordering::Relaxed) < 6;

        while count < buf.len() {
            let ch_opt = console_getchar();
            let mut ch = match ch_opt {
                Some(c) => c,
                None => break,
            };

            // input mapping
            if (term.c_iflag & IGNCR) != 0 && ch == b'\r' {
                continue; // drop CR
            }
            if (term.c_iflag & ICRNL) != 0 && ch == b'\r' {
                ch = b'\n';
            } else if (term.c_iflag & INLCR) != 0 && ch == b'\n' {
                ch = b'\r';
            }

            // echo
            if do_echo {
                console_putchar(ch);
            }

            buf[count] = ch;
            count += 1;

            if log_once && count == 1 {
                crate::pr_info!(
                    "[STDIO] read first byte: 0x{:02x} (canonical={})",
                    ch,
                    canonical
                );
            }

            if !canonical || ch == b'\n' {
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
    fn as_any(&self) -> &dyn core::any::Any {
        self
    }
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
        // 将整个缓冲区作为字符串输出，在一个锁内完成
        if let Ok(s) = core::str::from_utf8(buf) {
            crate::console::write_str(s);
        } else {
            // 如果不是有效 UTF-8，逐字节输出
            for &byte in buf {
                crate::console::putchar(byte);
            }
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
    fn as_any(&self) -> &dyn core::any::Any {
        self
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
        // 将整个缓冲区作为字符串输出，在一个锁内完成
        if let Ok(s) = core::str::from_utf8(buf) {
            crate::console::write_str(s);
        } else {
            // 如果不是有效 UTF-8，逐字节输出
            for &byte in buf {
                crate::console::putchar(byte);
            }
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
    fn as_any(&self) -> &dyn core::any::Any {
        self
    }
}

/// 通用的 stdio ioctl 实现
fn stdio_ioctl(request: u32, arg: usize) -> Result<isize, FsError> {
    use crate::arch::trap::SumGuard;
    use crate::uapi::errno::{EINVAL, ENOTTY};
    use crate::uapi::ioctl::*;

    match request {
        TCGETS => {
            if arg == 0 {
                return Ok(-EINVAL as isize);
            }

            unsafe {
                let _guard = SumGuard::new();
                let termios_ptr = arg as *mut Termios;
                if termios_ptr.is_null() {
                    return Ok(-EINVAL as isize);
                }

                // 清零结构体（包括 padding），避免泄露内核栈数据
                core::ptr::write_bytes(termios_ptr as *mut u8, 0, core::mem::size_of::<Termios>());

                // 返回保存的 termios 设置
                let termios = *STDIO_TERMIOS.lock();
                core::ptr::write_volatile(termios_ptr, termios);

                // 调试：打印返回的 termios 内容
                crate::pr_debug!(
                    "TCGETS: returning termios: iflag={:#x}, oflag={:#x}, cflag={:#x}, lflag={:#x}, ispeed={:#x}, ospeed={:#x}",
                    termios.c_iflag,
                    termios.c_oflag,
                    termios.c_cflag,
                    termios.c_lflag,
                    termios.c_ispeed,
                    termios.c_ospeed
                );
            }
            Ok(0)
        }

        TCSETS | TCSETSW | TCSETSF => {
            if arg == 0 {
                return Ok(-EINVAL as isize);
            }

            unsafe {
                let _guard = SumGuard::new();
                let termios_ptr = arg as *const Termios;
                if termios_ptr.is_null() {
                    return Ok(-EINVAL as isize);
                }

                // 读取新的 termios 设置并保存
                let new_termios = core::ptr::read_volatile(termios_ptr);

                // 调试：打印接收到的 termios 内容
                crate::pr_debug!(
                    "TCSETS: received termios: iflag={:#x}, oflag={:#x}, cflag={:#x}, lflag={:#x}, ispeed={:#x}, ospeed={:#x}",
                    new_termios.c_iflag,
                    new_termios.c_oflag,
                    new_termios.c_cflag,
                    new_termios.c_lflag,
                    new_termios.c_ispeed,
                    new_termios.c_ospeed
                );

                *STDIO_TERMIOS.lock() = new_termios;
            }
            Ok(0)
        }

        TIOCGWINSZ => {
            if arg == 0 {
                return Ok(-EINVAL as isize);
            }

            unsafe {
                let _guard = SumGuard::new();
                let winsize_ptr = arg as *mut crate::uapi::ioctl::WinSize;
                if winsize_ptr.is_null() {
                    return Ok(-EINVAL as isize);
                }

                // 清零结构体（包括 padding），避免泄露内核栈数据
                core::ptr::write_bytes(
                    winsize_ptr as *mut u8,
                    0,
                    core::mem::size_of::<crate::uapi::ioctl::WinSize>(),
                );

                // 返回保存的窗口大小
                let winsize = *STDIO_WINSIZE.lock();
                core::ptr::write_volatile(winsize_ptr, winsize);

                crate::pr_debug!(
                    "TIOCGWINSZ: returning {}x{} ({}x{} pixels)",
                    winsize.ws_row,
                    winsize.ws_col,
                    winsize.ws_xpixel,
                    winsize.ws_ypixel
                );
            }
            Ok(0)
        }

        TIOCSWINSZ => {
            if arg == 0 {
                return Ok(-EINVAL as isize);
            }

            unsafe {
                let _guard = SumGuard::new();
                let winsize_ptr = arg as *const crate::uapi::ioctl::WinSize;
                if winsize_ptr.is_null() {
                    return Ok(-EINVAL as isize);
                }

                // 读取新的窗口大小并保存
                let new_winsize = core::ptr::read_volatile(winsize_ptr);
                *STDIO_WINSIZE.lock() = new_winsize;

                crate::pr_debug!(
                    "TIOCSWINSZ: set to {}x{} ({}x{} pixels)",
                    new_winsize.ws_row,
                    new_winsize.ws_col,
                    new_winsize.ws_xpixel,
                    new_winsize.ws_ypixel
                );
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
