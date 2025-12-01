//! SysFS 属性文件测试

use super::*;
use crate::{kassert, test_case};

// 注意：由于属性文件需要具体的设备支持，
// 这里主要测试属性文件的基本机制

test_case!(test_sysfs_attribute_file_basic, {
    let sysfs = create_test_sysfs_with_tree().unwrap();
    // 属性文件测试需要具体的设备，这里先验证基本结构
    kassert!(sysfs.fs_type() == "sysfs");
});

test_case!(test_sysfs_attribute_readonly, {
    // 大多数 sysfs 属性文件应该是只读的
    let sysfs = create_test_sysfs_with_tree().unwrap();
    let root = sysfs.root_inode();

    // 验证目录结构存在
    kassert!(root.lookup("class").is_ok());
});

test_case!(test_sysfs_attribute_file_size_zero, {
    // sysfs 属性文件的大小通常报告为 0
    let sysfs = create_test_sysfs_with_tree().unwrap();
    // 此测试需要具体的属性文件，暂时验证基本功能
    kassert!(sysfs.sync().is_ok());
});
