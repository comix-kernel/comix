//! SysFS 基础功能测试

use super::*;
use crate::{kassert, test_case};

test_case!(test_sysfs_create, {
    let sysfs = create_test_sysfs();
    kassert!(sysfs.fs_type() == "sysfs");
});

test_case!(test_sysfs_root_inode, {
    let sysfs = create_test_sysfs();
    let root = sysfs.root_inode();

    let metadata = root.metadata().unwrap();
    kassert!(metadata.inode_type == InodeType::Directory);
    kassert!(metadata.mode.bits() & 0o777 == 0o555); // dr-xr-xr-x
});

test_case!(test_sysfs_sync, {
    let sysfs = create_test_sysfs();
    // sysfs 是纯内存文件系统，sync 应该总是成功
    let result = sysfs.sync();
    kassert!(result.is_ok());
});

test_case!(test_sysfs_statfs, {
    let sysfs = create_test_sysfs();
    let statfs = sysfs.statfs().unwrap();

    kassert!(statfs.block_size == 4096);
    kassert!(statfs.total_blocks == 0);
    kassert!(statfs.free_blocks == 0);
    kassert!(statfs.max_filename_len == 255);
});

test_case!(test_sysfs_root_empty_initially, {
    let sysfs = create_test_sysfs();
    let root = sysfs.root_inode();

    // 未初始化的 sysfs 根目录应该可以列出
    let entries = root.readdir();
    kassert!(entries.is_ok());
});

test_case!(test_sysfs_root_metadata, {
    let sysfs = create_test_sysfs();
    let root = sysfs.root_inode();

    let metadata = root.metadata().unwrap();
    kassert!(metadata.uid == 0);
    kassert!(metadata.gid == 0);
    kassert!(metadata.nlinks >= 2);
    kassert!(metadata.size == 0);
});

test_case!(test_sysfs_root_is_directory, {
    let sysfs = create_test_sysfs();
    let root = sysfs.root_inode();

    let metadata = root.metadata().unwrap();
    kassert!(metadata.inode_type == InodeType::Directory);

    // 目录不应该支持 read_at
    let mut buf = [0u8; 10];
    let result = root.read_at(0, &mut buf);
    kassert!(result.is_err());
});

test_case!(test_sysfs_root_readonly, {
    let sysfs = create_test_sysfs();
    let root = sysfs.root_inode();

    // 根目录应该是只读的（不能直接创建文件）
    let result = root.create("test.txt", FileMode::from_bits_truncate(0o644));
    kassert!(result.is_err());
});

test_case!(test_sysfs_multiple_instances, {
    let sysfs1 = create_test_sysfs();
    let sysfs2 = create_test_sysfs();

    kassert!(sysfs1.fs_type() == "sysfs");
    kassert!(sysfs2.fs_type() == "sysfs");

    let root1 = sysfs1.root_inode();
    let root2 = sysfs2.root_inode();

    let meta1 = root1.metadata().unwrap();
    let meta2 = root2.metadata().unwrap();

    kassert!(meta1.inode_no != meta2.inode_no);
});
