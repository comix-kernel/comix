//! 串行设备驱动模块
//!
//! 包含各种串行设备驱动程序的实现模块。

use super::Driver;

pub mod keyboard;
pub mod uart16550;
pub mod virtio_console;

/// 串行设备驱动程序特征
pub trait SerialDriver: Driver {
    /// 从 tty 读取一个字节
    fn read(&self) -> u8;

    /// 向 tty 写入数据
    fn write(&self, data: &[u8]);

    /// 尝试读取一个字节，如果没有数据则返回 None
    fn try_read(&self) -> Option<u8> {
        Some(self.read())
    }
}
