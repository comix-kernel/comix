//! 块设备模块
//!
//! 包含块设备相关的驱动接口和实现

use super::Driver;

pub mod partition;
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
    fn read_block(&self, block_id: usize, buf: &mut [u8]) -> bool;

    /// 连续读取多个块。
    ///
    /// 默认实现按单块循环，具体驱动可以覆盖为一次设备请求以减少 I/O 往返。
    fn read_blocks(&self, start_block: usize, buf: &mut [u8]) -> bool {
        let block_size = self.block_size();
        if block_size == 0 || !buf.len().is_multiple_of(block_size) {
            return false;
        }

        for (idx, block) in buf.chunks_exact_mut(block_size).enumerate() {
            if !self.read_block(start_block + idx, block) {
                return false;
            }
        }
        true
    }

    /// 写入块设备数据
    /// # 参数：
    /// * `block_id` - 块设备的块号
    /// * `buf` - 包含要写入数据的缓冲区
    /// # 返回值：
    /// 如果写入成功则返回 true，否则返回 false
    fn write_block(&self, block_id: usize, buf: &[u8]) -> bool;

    /// 连续写入多个块。
    ///
    /// 默认实现按单块循环，具体驱动可以覆盖为一次设备请求以减少 I/O 往返。
    fn write_blocks(&self, start_block: usize, buf: &[u8]) -> bool {
        let block_size = self.block_size();
        if block_size == 0 || !buf.len().is_multiple_of(block_size) {
            return false;
        }

        for (idx, block) in buf.chunks_exact(block_size).enumerate() {
            if !self.write_block(start_block + idx, block) {
                return false;
            }
        }
        true
    }

    /// 刷新到磁盘
    /// # 返回值：
    /// 如果刷新成功则返回 true，否则返回 false
    fn flush(&self) -> bool;

    /// 获取块大小（字节）
    /// # 返回值：
    /// 块大小
    fn block_size(&self) -> usize;

    /// 获取总块数
    /// # 返回值：
    /// 总块数
    fn total_blocks(&self) -> usize;
}
