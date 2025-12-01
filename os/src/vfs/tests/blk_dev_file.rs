//! 块设备文件测试
//!
//! 注意：BlockDeviceFile 需要通过 Dentry 创建，并且需要设备驱动注册。
//! 这里主要测试基本的类型和接口。

use super::*;
use crate::device::block::BlockDriver;
use crate::{kassert, test_case};

// 由于 BlockDeviceFile 需要完整的设备注册流程，
// 这里主要测试辅助函数和基本逻辑

test_case!(test_blk_dev_helper_create_ramdisk, {
    // 测试创建测试用 RamDisk
    let ramdisk = create_test_ramdisk(10); // 10 blocks
    let driver: &dyn BlockDriver = &*ramdisk;

    kassert!(driver.block_size() == 512);
    kassert!(driver.total_blocks() == 10);
});

test_case!(test_blk_dev_helper_ramdisk_operations, {
    let ramdisk = create_test_ramdisk(4);
    let driver: &dyn BlockDriver = &*ramdisk;

    // 测试写入
    let write_data = [0xAA; 512];
    let result = driver.write_block(0, &write_data);
    kassert!(result);

    // 测试读取
    let mut read_data = [0u8; 512];
    let result = driver.read_block(0, &mut read_data);
    kassert!(result);
    kassert!(read_data[0] == 0xAA);
    kassert!(read_data[511] == 0xAA);
});

test_case!(test_blk_dev_helper_multiple_blocks, {
    let ramdisk = create_test_ramdisk(10);
    let driver: &dyn BlockDriver = &*ramdisk;

    // 写入多个块
    for i in 0..5 {
        let data = [i as u8; 512];
        kassert!(driver.write_block(i as usize, &data));
    }

    // 验证每个块
    for i in 0..5 {
        let mut buf = [0u8; 512];
        kassert!(driver.read_block(i as usize, &mut buf));
        kassert!(buf[0] == i as u8);
    }
});

test_case!(test_blk_dev_boundary_check, {
    let ramdisk = create_test_ramdisk(2);
    let driver: &dyn BlockDriver = &*ramdisk;

    // 越界读取应该失败
    let mut buf = [0u8; 512];
    let result = driver.read_block(10, &mut buf);
    kassert!(!result);

    // 越界写入应该失败
    let data = [0xBB; 512];
    let result = driver.write_block(10, &data);
    kassert!(!result);
});
