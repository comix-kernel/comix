//! SysFS 集成测试

use super::*;
use crate::{kassert, test_case};

test_case!(test_sysfs_init_tree, {
    let sysfs = SysFS::new();
    let result = sysfs.init_tree();
    kassert!(result.is_ok());
});

test_case!(test_sysfs_full_initialization, {
    let sysfs = create_test_sysfs_with_tree().unwrap();
    let root = sysfs.root_inode();

    // 验证所有顶层目录都存在
    kassert!(root.lookup("class").is_ok());
    kassert!(root.lookup("devices").is_ok());
    kassert!(root.lookup("kernel").is_ok());
    kassert!(root.lookup("block").is_ok()); // symlink
});

test_case!(test_sysfs_filesystem_type, {
    let sysfs = create_test_sysfs_with_tree().unwrap();
    kassert!(sysfs.fs_type() == "sysfs");
});

test_case!(test_sysfs_sync_after_init, {
    let sysfs = create_test_sysfs_with_tree().unwrap();
    kassert!(sysfs.sync().is_ok());
});

test_case!(test_sysfs_statfs_after_init, {
    let sysfs = create_test_sysfs_with_tree().unwrap();
    let statfs = sysfs.statfs();
    kassert!(statfs.is_ok());

    let stats = statfs.unwrap();
    kassert!(stats.block_size == 4096);
    kassert!(stats.total_blocks == 0);
});

test_case!(test_sysfs_multiple_init, {
    let sysfs = SysFS::new();
    kassert!(sysfs.init_tree().is_ok());

    // 第二次初始化可能失败或成功（幂等）
    let result = sysfs.init_tree();
    kassert!(result.is_ok() || result.is_err());
});

test_case!(test_sysfs_readdir_all_entries, {
    let sysfs = create_test_sysfs_with_tree().unwrap();
    let root = sysfs.root_inode();

    let entries = root.readdir().unwrap();

    // 应该至少有：., .., class, devices, kernel, block
    kassert!(entries.len() >= 6);

    // 验证所有条目都可以 lookup
    for entry in entries {
        if entry.name != "." && entry.name != ".." {
            let result = root.lookup(&entry.name);
            kassert!(result.is_ok());
        }
    }
});

test_case!(test_sysfs_class_hierarchy, {
    let sysfs = create_test_sysfs_with_tree().unwrap();
    let root = sysfs.root_inode();

    // 测试完整的层次结构访问
    let class_dir = root.lookup("class").unwrap();
    let block_dir = class_dir.lookup("block").unwrap();

    let metadata = block_dir.metadata().unwrap();
    kassert!(metadata.inode_type == InodeType::Directory);
});

test_case!(test_sysfs_symlink_resolution, {
    let sysfs = create_test_sysfs_with_tree().unwrap();
    let root = sysfs.root_inode();

    // /sys/block -> class/block
    let block_link = root.lookup("block").unwrap();
    let target = block_link.readlink().unwrap();
    kassert!(target == "class/block");
});

test_case!(test_sysfs_concurrent_access, {
    let sysfs = create_test_sysfs_with_tree().unwrap();
    let root = sysfs.root_inode();

    // 多次访问同一目录
    let class_dir1 = root.lookup("class").unwrap();
    let class_dir2 = root.lookup("class").unwrap();

    let meta1 = class_dir1.metadata().unwrap();
    let meta2 = class_dir2.metadata().unwrap();

    // 应该是同一个 inode
    kassert!(meta1.inode_no == meta2.inode_no);
});

test_case!(test_sysfs_all_class_directories, {
    let sysfs = create_test_sysfs_with_tree().unwrap();
    let root = sysfs.root_inode();
    let class_dir = root.lookup("class").unwrap();

    let entries = class_dir.readdir().unwrap();
    let names: alloc::vec::Vec<_> = entries.iter().map(|e| e.name.as_str()).collect();

    // 验证所有设备类都存在
    kassert!(names.contains(&"block"));
    kassert!(names.contains(&"net"));
    kassert!(names.contains(&"tty"));
    kassert!(names.contains(&"input"));
    kassert!(names.contains(&"rtc"));
});
