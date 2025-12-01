//! ProcFS 目录操作测试

use super::*;
use crate::{kassert, test_case};

test_case!(test_procfs_root_readdir, {
    let procfs = create_test_procfs_with_tree().unwrap();
    let root = procfs.root_inode();

    let entries = root.readdir();
    kassert!(entries.is_ok());

    let dir_entries = entries.unwrap();
    // 应该至少包含 . 和 ..
    kassert!(dir_entries.len() >= 2);

    // 检查是否包含预期的文件
    let names: alloc::vec::Vec<_> = dir_entries.iter().map(|e| e.name.as_str()).collect();
    kassert!(names.contains(&"."));
    kassert!(names.contains(&".."));
});

test_case!(test_procfs_readdir_contains_files, {
    let procfs = create_test_procfs_with_tree().unwrap();
    let root = procfs.root_inode();

    let entries = root.readdir().unwrap();
    let names: alloc::vec::Vec<_> = entries.iter().map(|e| e.name.as_str()).collect();

    // 应该包含初始化的文件
    kassert!(names.contains(&"meminfo"));
    kassert!(names.contains(&"uptime"));
    kassert!(names.contains(&"cpuinfo"));
    kassert!(names.contains(&"mounts"));
    kassert!(names.contains(&"self"));
});

test_case!(test_procfs_lookup_nonexistent, {
    let procfs = create_test_procfs_with_tree().unwrap();
    let root = procfs.root_inode();

    let result = root.lookup("nonexistent");
    kassert!(result.is_err());
});

test_case!(test_procfs_lookup_meminfo, {
    let procfs = create_test_procfs_with_tree().unwrap();
    let root = procfs.root_inode();

    let meminfo = root.lookup("meminfo");
    kassert!(meminfo.is_ok());

    let inode = meminfo.unwrap();
    let metadata = inode.metadata().unwrap();
    kassert!(metadata.inode_type == InodeType::File);
});

test_case!(test_procfs_lookup_self, {
    let procfs = create_test_procfs_with_tree().unwrap();
    let root = procfs.root_inode();

    let self_link = root.lookup("self");
    kassert!(self_link.is_ok());

    let inode = self_link.unwrap();
    let metadata = inode.metadata().unwrap();
    kassert!(metadata.inode_type == InodeType::Symlink);
});

test_case!(test_procfs_directory_not_writable, {
    let procfs = create_test_procfs_with_tree().unwrap();
    let root = procfs.root_inode();

    // 不应该能够创建新文件
    let result = root.create("newfile", FileMode::from_bits_truncate(0o644));
    kassert!(result.is_err());

    // 不应该能够创建新目录
    let result = root.mkdir("newdir", FileMode::from_bits_truncate(0o755));
    kassert!(result.is_err());
});

test_case!(test_procfs_directory_not_deletable, {
    let procfs = create_test_procfs_with_tree().unwrap();
    let root = procfs.root_inode();

    // 不应该能够删除文件
    let result = root.unlink("meminfo");
    kassert!(result.is_err());

    // 不应该能够删除符号链接
    let result = root.unlink("self");
    kassert!(result.is_err());
});

test_case!(test_procfs_readdir_entry_types, {
    let procfs = create_test_procfs_with_tree().unwrap();
    let root = procfs.root_inode();

    let entries = root.readdir().unwrap();

    for entry in entries {
        // 验证每个条目都有有效的类型
        kassert!(
            entry.inode_type == InodeType::File
                || entry.inode_type == InodeType::Directory
                || entry.inode_type == InodeType::Symlink
        );

        // 验证名称非空
        kassert!(entry.name.len() > 0);

        // 验证 inode 号非零
        kassert!(entry.inode_no > 0);
    }
});
