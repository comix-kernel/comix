//! SysFS 设备构建器测试

use super::*;
use crate::{kassert, test_case};

// 设备构建器测试
// 这些测试验证设备树构建逻辑

test_case!(test_sysfs_builders_block_devices, {
    let sysfs = create_test_sysfs_with_tree().unwrap();
    let root = sysfs.root_inode();

    // 验证块设备类存在
    let class_dir = root.lookup("class").unwrap();
    let block_dir = class_dir.lookup("block");
    kassert!(block_dir.is_ok());
});

test_case!(test_sysfs_builders_net_devices, {
    let sysfs = create_test_sysfs_with_tree().unwrap();
    let root = sysfs.root_inode();

    let class_dir = root.lookup("class").unwrap();
    let net_dir = class_dir.lookup("net");
    kassert!(net_dir.is_ok());
});

test_case!(test_sysfs_builders_tty_devices, {
    let sysfs = create_test_sysfs_with_tree().unwrap();
    let root = sysfs.root_inode();

    let class_dir = root.lookup("class").unwrap();
    let tty_dir = class_dir.lookup("tty");
    kassert!(tty_dir.is_ok());
});

test_case!(test_sysfs_builders_input_devices, {
    let sysfs = create_test_sysfs_with_tree().unwrap();
    let root = sysfs.root_inode();

    let class_dir = root.lookup("class").unwrap();
    let input_dir = class_dir.lookup("input");
    kassert!(input_dir.is_ok());
});

test_case!(test_sysfs_builders_rtc_devices, {
    let sysfs = create_test_sysfs_with_tree().unwrap();
    let root = sysfs.root_inode();

    let class_dir = root.lookup("class").unwrap();
    let rtc_dir = class_dir.lookup("rtc");
    kassert!(rtc_dir.is_ok());
});

test_case!(test_sysfs_builders_kernel_info, {
    let sysfs = create_test_sysfs_with_tree().unwrap();
    let root = sysfs.root_inode();

    // 验证内核信息目录存在
    let kernel_dir = root.lookup("kernel");
    kassert!(kernel_dir.is_ok());
});

test_case!(test_sysfs_builders_platform_devices, {
    let sysfs = create_test_sysfs_with_tree().unwrap();
    let root = sysfs.root_inode();

    // 验证设备目录存在
    let devices_dir = root.lookup("devices");
    kassert!(devices_dir.is_ok());
});
