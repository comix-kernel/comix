use crate::device::Driver;
use crate::sync::SpinLock;
use crate::uapi::ioctl::Termios;
use crate::vfs::dev::{major, minor};
use crate::vfs::devno::{chrdev_major, get_chrdev_driver, misc_minor};
use crate::vfs::{Dentry, File, FsError, Inode, InodeMetadata, OpenFlags, SeekWhence};
use alloc::sync::Arc;

/// 字符设备文件
pub struct CharDeviceFile {
    /// 关联的 dentry
    pub dentry: Arc<Dentry>,

    /// 关联的 inode
    pub inode: Arc<dyn Inode>,

    /// 设备号
    dev: u64,

    /// 设备驱动（缓存）
    driver: Option<Arc<dyn Driver>>,

    /// 打开标志位
    pub flags: OpenFlags,

    /// 偏移量（某些字符设备可能需要）
    offset: SpinLock<usize>,

    /// 终端属性（用于 TTY 设备）
    termios: SpinLock<Termios>,

    /// 终端窗口大小（用于 TTY 设备）
    winsize: SpinLock<crate::uapi::ioctl::WinSize>,
}

impl CharDeviceFile {
    /// 创建新的字符设备文件
    ///
    /// # 参数
    /// - `dentry`: 设备文件的 dentry
    /// - `flags`: 打开标志
    ///
    /// # 返回
    /// - `Ok(CharDeviceFile)`: 成功
    /// - `Err(FsError::NoDevice)`: 找不到对应的驱动
    pub fn new(dentry: Arc<Dentry>, flags: OpenFlags) -> Result<Self, FsError> {
        let inode = dentry.inode.clone();
        let metadata = inode.metadata()?;
        let dev = metadata.rdev;

        // 通过硬编码规则查找驱动
        // 内存设备（major=1）会返回 None，在 read/write 中直接处理
        let driver = get_chrdev_driver(dev);

        // 检查设备是否支持
        let maj = major(dev);
        if driver.is_none() && maj != chrdev_major::MEM {
            // 既不是内存设备，也找不到驱动
            return Err(FsError::NoDevice);
        }

        Ok(Self {
            dentry,
            inode,
            dev,
            driver,
            flags,
            offset: SpinLock::new(0),
            termios: SpinLock::new(Termios::default()),
            winsize: SpinLock::new(crate::uapi::ioctl::WinSize {
                ws_row: 24,
                ws_col: 80,
                ws_xpixel: 0,
                ws_ypixel: 0,
            }),
        })
    }

    /// 处理内存设备的读操作
    fn mem_device_read(&self, buf: &mut [u8]) -> Result<usize, FsError> {
        let min = minor(self.dev);
        match min {
            3 => {
                // /dev/null: 总是返回 0
                Ok(0)
            }
            5 => {
                // /dev/zero: 填充零
                buf.fill(0);
                Ok(buf.len())
            }
            8 | 9 => {
                // /dev/random, /dev/urandom: 简单实现（使用时间戳）
                use crate::arch::timer::get_ticks;
                let mut seed = get_ticks() as u32;
                for byte in buf.iter_mut() {
                    // 简单的 LCG 随机数生成器
                    seed = seed.wrapping_mul(1103515245).wrapping_add(12345);
                    *byte = (seed >> 16) as u8;
                }
                Ok(buf.len())
            }
            _ => Err(FsError::NoDevice),
        }
    }

    /// 处理内存设备的写操作
    fn mem_device_write(&self, buf: &[u8]) -> Result<usize, FsError> {
        let min = minor(self.dev);
        match min {
            3 | 5 => {
                // /dev/null, /dev/zero: 丢弃所有数据
                Ok(buf.len())
            }
            _ => Err(FsError::NoDevice),
        }
    }
}

impl File for CharDeviceFile {
    fn readable(&self) -> bool {
        self.flags.readable()
    }

    fn writable(&self) -> bool {
        self.flags.writable()
    }

    fn read(&self, buf: &mut [u8]) -> Result<usize, FsError> {
        if !self.readable() {
            return Err(FsError::PermissionDenied);
        }

        let maj = major(self.dev);

        // 内存设备特殊处理
        if maj == chrdev_major::MEM {
            return self.mem_device_read(buf);
        }

        // 其他设备：委托给驱动
        if let Some(ref driver) = self.driver {
            if let Some(serial) = driver.as_serial() {
                let is_nonblock = self.flags.contains(OpenFlags::O_NONBLOCK);

                if is_nonblock {
                    // 非阻塞模式：尝试读取，没有数据则返回 EAGAIN
                    if let Some(byte) = serial.try_read() {
                        buf[0] = byte;
                        let mut count = 1;
                        // 尽可能多读，但不阻塞
                        while count < buf.len() {
                            if let Some(b) = serial.try_read() {
                                buf[count] = b;
                                count += 1;
                            } else {
                                break;
                            }
                        }
                        Ok(count)
                    } else {
                        Err(FsError::WouldBlock)
                    }
                } else {
                    // 阻塞模式：至少读取一个字节
                    buf[0] = serial.read();
                    let mut count = 1;
                    // 尝试读取更多，但不阻塞
                    while count < buf.len() {
                        if let Some(b) = serial.try_read() {
                            buf[count] = b;
                            count += 1;
                        } else {
                            break;
                        }
                    }
                    Ok(count)
                }
            } else {
                Err(FsError::NotSupported)
            }
        } else {
            Err(FsError::NoDevice)
        }
    }

    fn write(&self, buf: &[u8]) -> Result<usize, FsError> {
        if !self.writable() {
            return Err(FsError::PermissionDenied);
        }

        let maj = major(self.dev);

        // 内存设备特殊处理
        if maj == chrdev_major::MEM {
            return self.mem_device_write(buf);
        }

        // 其他设备：委托给驱动
        if let Some(ref driver) = self.driver {
            if let Some(serial) = driver.as_serial() {
                serial.write(buf);
                Ok(buf.len())
            } else {
                Err(FsError::NotSupported)
            }
        } else {
            Err(FsError::NoDevice)
        }
    }

    fn metadata(&self) -> Result<InodeMetadata, FsError> {
        self.inode.metadata()
    }

    fn lseek(&self, offset: isize, whence: SeekWhence) -> Result<usize, FsError> {
        // 大多数字符设备不支持 seek
        // 但某些设备（如 /dev/mem）可能需要
        Err(FsError::NotSupported)
    }

    fn offset(&self) -> usize {
        *self.offset.lock()
    }

    fn flags(&self) -> OpenFlags {
        self.flags.clone()
    }

    fn inode(&self) -> Result<Arc<dyn Inode>, FsError> {
        Ok(self.inode.clone())
    }

    fn dentry(&self) -> Result<Arc<Dentry>, FsError> {
        Ok(self.dentry.clone())
    }

    fn ioctl(&self, request: u32, arg: usize) -> Result<isize, FsError> {
        use crate::arch::trap::SumGuard;
        use crate::uapi::errno::{EINVAL, ENOTTY};
        use crate::uapi::ioctl::*;

        let maj = major(self.dev);

        // 根据设备类型分发 ioctl
        match maj {
            chrdev_major::CONSOLE => {
                // 终端 ioctl
                self.console_ioctl(request, arg)
            }
            chrdev_major::MISC => {
                // MISC 设备 ioctl (包括 RTC)
                self.misc_ioctl(request, arg)
            }
            _ => Err(FsError::NotSupported),
        }
    }
    fn as_any(&self) -> &dyn core::any::Any {
        self
    }
}

impl CharDeviceFile {
    /// 控制台设备 ioctl 处理
    fn console_ioctl(&self, request: u32, arg: usize) -> Result<isize, FsError> {
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
                    core::ptr::write_bytes(
                        termios_ptr as *mut u8,
                        0,
                        core::mem::size_of::<Termios>(),
                    );

                    // 返回保存的 termios 设置
                    let termios = *self.termios.lock();
                    core::ptr::write_volatile(termios_ptr, termios);
                }
                Ok(0)
            }

            TCSETS | TCSETSW | TCSETSF => {
                if arg == 0 {
                    return Ok(-EINVAL as isize);
                }

                {
                    let _guard = SumGuard::new();
                    let termios_ptr = arg as *const Termios;
                    if termios_ptr.is_null() {
                        return Ok(-EINVAL as isize);
                    }

                    unsafe {
                        // 读取新的 termios 设置并保存
                        let new_termios = core::ptr::read_volatile(termios_ptr);
                        *self.termios.lock() = new_termios;
                    }
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
                    let winsize = *self.winsize.lock();
                    core::ptr::write_volatile(winsize_ptr, winsize);
                }
                Ok(0)
            }

            TIOCSWINSZ => {
                if arg == 0 {
                    return Ok(-EINVAL as isize);
                }

                {
                    let _guard = SumGuard::new();
                    let winsize_ptr = arg as *const crate::uapi::ioctl::WinSize;
                    if winsize_ptr.is_null() {
                        return Ok(-EINVAL as isize);
                    }

                    unsafe {
                        // 读取新的窗口大小并保存
                        let new_winsize = core::ptr::read_volatile(winsize_ptr);
                        *self.winsize.lock() = new_winsize;
                    }
                }
                Ok(0)
            }

            // 其他 ioctl 命令不支持
            _ => Ok(-ENOTTY as isize),
        }
    }

    /// MISC 设备 ioctl 处理
    fn misc_ioctl(&self, request: u32, arg: usize) -> Result<isize, FsError> {
        use crate::arch::trap::SumGuard;
        use crate::uapi::errno::EINVAL;
        use crate::uapi::ioctl::*;
        use crate::vfs::dev::minor;

        let min = minor(self.dev);

        // RTC 设备 (minor=135)
        if min == misc_minor::RTC {
            match request {
                RTC_RD_TIME => {
                    if arg == 0 {
                        return Ok(-EINVAL as isize);
                    }

                    // 通过驱动获取时间
                    if let Some(ref driver) = self.driver {
                        if let Some(rtc) = driver.as_rtc() {
                            let dt = rtc.read_datetime();

                            unsafe {
                                let _guard = SumGuard::new();
                                let rtc_time_ptr = arg as *mut RtcTime;
                                if rtc_time_ptr.is_null() {
                                    return Ok(-EINVAL as isize);
                                }

                                // 清零结构体
                                core::ptr::write_bytes(
                                    rtc_time_ptr as *mut u8,
                                    0,
                                    core::mem::size_of::<RtcTime>(),
                                );

                                // 填充时间结构体
                                let rtc_time = RtcTime {
                                    tm_sec: dt.second as i32,
                                    tm_min: dt.minute as i32,
                                    tm_hour: dt.hour as i32,
                                    tm_mday: dt.day as i32,
                                    tm_mon: (dt.month - 1) as i32, // Linux 月份是 0-based
                                    tm_year: (dt.year - 1900) as i32,
                                    tm_wday: 0, // 未计算
                                    tm_yday: 0, // 未计算
                                    tm_isdst: 0,
                                };

                                core::ptr::write_volatile(rtc_time_ptr, rtc_time);
                            }
                            return Ok(0);
                        }
                    }
                    Err(FsError::NoDevice)
                }
                _ => Err(FsError::NotSupported),
            }
        } else {
            Err(FsError::NotSupported)
        }
    }
}
