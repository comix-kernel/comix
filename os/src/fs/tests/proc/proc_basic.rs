//! ProcFS 基础功能测试

use super::*;
use crate::{kassert, test_case};

test_case!(test_procfs_create, {
    let procfs = create_test_procfs();
    kassert!(procfs.fs_type() == "proc");
});

test_case!(test_procfs_root_inode, {
    let procfs = create_test_procfs();
    let root = procfs.root_inode();

    let metadata = root.metadata().unwrap();
    kassert!(metadata.inode_type == InodeType::Directory);
    kassert!(metadata.mode.bits() & 0o777 == 0o555); // dr-xr-xr-x
});

test_case!(test_procfs_sync, {
    let procfs = create_test_procfs();
    // proc 是纯内存文件系统，sync 应该总是成功
    let result = procfs.sync();
    kassert!(result.is_ok());
});

test_case!(test_procfs_statfs, {
    let procfs = create_test_procfs();
    let statfs = procfs.statfs().unwrap();

    // proc 文件系统的特征
    kassert!(statfs.block_size == 4096);
    kassert!(statfs.total_blocks == 0);
    kassert!(statfs.free_blocks == 0);
    kassert!(statfs.total_inodes == 0);
    kassert!(statfs.max_filename_len == 255);
});

test_case!(test_procfs_root_empty_initially, {
    let procfs = create_test_procfs();
    let root = procfs.root_inode();

    // 未初始化的 procfs 根目录应该可以列出（但可能为空）
    let entries = root.readdir();
    kassert!(entries.is_ok());
});

test_case!(test_procfs_root_metadata, {
    let procfs = create_test_procfs();
    let root = procfs.root_inode();

    let metadata = root.metadata().unwrap();
    kassert!(metadata.uid == 0);
    kassert!(metadata.gid == 0);
    kassert!(metadata.nlinks >= 2); // . 和可能的子目录
    kassert!(metadata.size == 0);
});

test_case!(test_procfs_root_is_directory, {
    let procfs = create_test_procfs();
    let root = procfs.root_inode();

    let metadata = root.metadata().unwrap();
    kassert!(metadata.inode_type == InodeType::Directory);

    // 目录不应该支持 read_at
    let mut buf = [0u8; 10];
    let result = root.read_at(0, &mut buf);
    kassert!(result.is_err());
});

test_case!(test_procfs_root_readonly, {
    let procfs = create_test_procfs();
    let root = procfs.root_inode();

    // 根目录应该是只读的
    let result = root.create("test.txt", FileMode::from_bits_truncate(0o644));
    kassert!(result.is_err());
});

test_case!(test_procfs_multiple_instances, {
    // 可以创建多个 procfs 实例
    let procfs1 = create_test_procfs();
    let procfs2 = create_test_procfs();

    kassert!(procfs1.fs_type() == "proc");
    kassert!(procfs2.fs_type() == "proc");

    // 每个实例有独立的根 inode
    let root1 = procfs1.root_inode();
    let root2 = procfs2.root_inode();

    let meta1 = root1.metadata().unwrap();
    let meta2 = root2.metadata().unwrap();

    kassert!(meta1.inode_no != meta2.inode_no);
});
