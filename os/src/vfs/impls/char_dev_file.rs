use crate::device::Driver;
use crate::sync::SpinLock;
use crate::vfs::{Dentry, File, FsError, Inode, InodeMetadata, OpenFlags, SeekWhence};
use crate::vfs::dev::{major, minor};
use crate::vfs::devno::{chrdev_major, get_chrdev_driver};
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
            // 这里需要根据具体驱动类型调用相应方法
            // 示例：串口设备
            unimplemented!()
            // if let Some(serial) = driver.as_serial() {
            //     // 假设 SerialDriver 有 read 方法
            //     // let n = serial.read(buf)?;
            //     // return Ok(n);

            //     // 临时实现：返回空
            //     Ok(0)
            // } else {
            //     Err(FsError::NotSupported)
            // }
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
            unimplemented!()
            // if let Some(serial) = driver.as_serial() {
            //     // 假设 SerialDriver 有 write 方法
            //     // let n = serial.write(buf)?;
            //     // return Ok(n);

            //     // 临时实现
            //     Ok(buf.len())
            // } else {
            //     Err(FsError::NotSupported)
            // }
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
}