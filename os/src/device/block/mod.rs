//! 块设备模块
//!
//! 包含块设备相关的驱动接口和实现

use super::Driver;

pub mod ram_disk;
pub mod virtio_blk;

/// 块设备驱动程序接口
pub trait BlockDriver: Driver {
    /// 读取块设备数据
    /// # 参数：
    /// * `block_id` - 块设备的块号
    /// * `buf` - 用于存储读取数据的缓冲区
    /// # 返回值：
    /// 如果读取成功则返回 true，否则返回 false
    fn read_block(&self, _block_id: usize, _buf: &mut [u8]) -> bool {
        unimplemented!("not a block driver")
    }

    /// 写入块设备数据
    /// # 参数：
    /// * `block_id` - 块设备的块号
    /// * `buf` - 包含要写入数据的缓冲区
    /// # 返回值：
    /// 如果写入成功则返回 true，否则返回 false
    fn write_block(&self, _block_id: usize, _buf: &[u8]) -> bool {
        unimplemented!("not a block driver")
    }

    /// 刷新到磁盘
    /// # 返回值：
    /// 如果刷新成功则返回 true，否则返回 false
    fn flush(&self) -> bool {
        unimplemented!("not a block driver")
    }

    /// 获取块大小（字节）
    /// # 返回值：
    /// 块大小
    fn block_size(&self) -> usize {
        unimplemented!("not a block driver")
    }

    /// 获取总块数
    /// # 返回值：
    /// 总块数
    fn total_blocks(&self) -> usize {
        unimplemented!("not a block driver")
    }
}
