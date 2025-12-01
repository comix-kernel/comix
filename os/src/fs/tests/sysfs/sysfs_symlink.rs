//! SysFS 符号链接测试

use super::*;
use crate::{kassert, test_case};

test_case!(test_sysfs_block_symlink_exists, {
    let sysfs = create_test_sysfs_with_tree().unwrap();
    let root = sysfs.root_inode();

    // /sys/block 应该是指向 class/block 的符号链接
    let block_link = root.lookup("block");
    kassert!(block_link.is_ok());

    let inode = block_link.unwrap();
    let metadata = inode.metadata().unwrap();
    kassert!(metadata.inode_type == InodeType::Symlink);
});

test_case!(test_sysfs_block_symlink_target, {
    let sysfs = create_test_sysfs_with_tree().unwrap();
    let root = sysfs.root_inode();

    let block_link = root.lookup("block").unwrap();
    let target = block_link.readlink();
    kassert!(target.is_ok());

    let target_path = target.unwrap();
    kassert!(target_path == "class/block");
});

test_case!(test_sysfs_symlink_metadata, {
    let sysfs = create_test_sysfs_with_tree().unwrap();
    let root = sysfs.root_inode();

    let block_link = root.lookup("block").unwrap();
    let metadata = block_link.metadata().unwrap();

    kassert!(metadata.inode_type == InodeType::Symlink);
    kassert!(metadata.nlinks == 1);
});

test_case!(test_sysfs_symlink_not_writable, {
    let sysfs = create_test_sysfs_with_tree().unwrap();
    let root = sysfs.root_inode();

    let block_link = root.lookup("block").unwrap();
    let result = block_link.write_at(0, b"test");
    kassert!(result.is_err());
});

test_case!(test_sysfs_symlink_not_readable_as_file, {
    let sysfs = create_test_sysfs_with_tree().unwrap();
    let root = sysfs.root_inode();

    let block_link = root.lookup("block").unwrap();
    let mut buf = [0u8; 10];
    let result = block_link.read_at(0, &mut buf);
    kassert!(result.is_err());
});
