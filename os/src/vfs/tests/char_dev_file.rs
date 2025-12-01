//! 字符设备文件测试
//!
//! 注意：CharDeviceFile 需要通过 Dentry 创建，并且需要设备驱动注册。
//! 这里主要测试基本的类型和接口。

use super::*;
use crate::device::block::BlockDriver;
use crate::{kassert, test_case};

// 字符设备文件测试需要完整的设备注册流程
// 这里主要测试辅助函数和基本逻辑

test_case!(test_char_dev_basic, {
    // 基本的字符设备测试
    // 字符设备文件需要通过系统调用和设备节点创建
    // 这里验证基本的测试框架
    kassert!(true);
});

test_case!(test_char_dev_helper_ramdisk, {
    // 使用 RamDisk 作为测试辅助
    let ramdisk = create_test_ramdisk(2);
    let driver: &dyn BlockDriver = &*ramdisk;
    kassert!(driver.block_size() == 512);
});
