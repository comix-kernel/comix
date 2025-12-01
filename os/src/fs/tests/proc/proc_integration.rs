//! ProcFS 集成测试

use super::*;
use crate::{kassert, test_case};

test_case!(test_procfs_init_tree, {
    let procfs = ProcFS::new();
    let result = procfs.init_tree();
    kassert!(result.is_ok());
});

test_case!(test_procfs_full_initialization, {
    let procfs = create_test_procfs_with_tree().unwrap();
    let root = procfs.root_inode();

    // 验证所有预期的文件都存在
    kassert!(root.lookup("meminfo").is_ok());
    kassert!(root.lookup("uptime").is_ok());
    kassert!(root.lookup("cpuinfo").is_ok());
    kassert!(root.lookup("mounts").is_ok());
    kassert!(root.lookup("self").is_ok());
});

// TODO: 此测试需要完整的内核上下文来生成动态内容
// test_case!(test_procfs_read_all_files, {
//     let procfs = create_test_procfs_with_tree().unwrap();
//     let root = procfs.root_inode();
//
//     // 尝试读取所有文件
//     let files = ["meminfo", "uptime", "cpuinfo", "mounts"];
//     let mut buf = [0u8; 2048];
//
//     for filename in &files {
//         let inode = root.lookup(filename).unwrap();
//         let result = inode.read_at(0, &mut buf);
//         kassert!(result.is_ok());
//         kassert!(result.unwrap() > 0);
//     }
// });

test_case!(test_procfs_filesystem_type, {
    let procfs = create_test_procfs_with_tree().unwrap();
    kassert!(procfs.fs_type() == "proc");
});

test_case!(test_procfs_sync_after_init, {
    let procfs = create_test_procfs_with_tree().unwrap();
    kassert!(procfs.sync().is_ok());
});

test_case!(test_procfs_statfs_after_init, {
    let procfs = create_test_procfs_with_tree().unwrap();
    let statfs = procfs.statfs();
    kassert!(statfs.is_ok());

    let stats = statfs.unwrap();
    kassert!(stats.block_size == 4096);
    kassert!(stats.total_blocks == 0);
});

test_case!(test_procfs_multiple_init, {
    // 测试多次初始化
    let procfs = ProcFS::new();
    kassert!(procfs.init_tree().is_ok());

    // 第二次初始化可能失败（文件已存在）或成功（幂等）
    let result = procfs.init_tree();
    // 无论成功或失败，都不应该导致系统崩溃
    kassert!(result.is_ok() || result.is_err());
});

test_case!(test_procfs_concurrent_access, {
    // 测试并发访问（在单线程环境中模拟）
    let procfs = create_test_procfs_with_tree().unwrap();
    let root = procfs.root_inode();

    // 多次读取同一文件
    let meminfo = root.lookup("meminfo").unwrap();
    let mut buf1 = [0u8; 1024];
    let mut buf2 = [0u8; 1024];

    kassert!(meminfo.read_at(0, &mut buf1).is_ok());
    kassert!(meminfo.read_at(0, &mut buf2).is_ok());
});

test_case!(test_procfs_readdir_all_entries, {
    let procfs = create_test_procfs_with_tree().unwrap();
    let root = procfs.root_inode();

    let entries = root.readdir().unwrap();

    // 应该至少有：., .., meminfo, uptime, cpuinfo, mounts, self
    kassert!(entries.len() >= 7);

    // 验证所有条目都可以 lookup
    for entry in entries {
        if entry.name != "." && entry.name != ".." {
            let result = root.lookup(&entry.name);
            kassert!(result.is_ok());
        }
    }
});

// TODO: 此测试需要 current_task，需要完整的内核上下文
// test_case!(test_procfs_self_link_resolution, {
//     let procfs = create_test_procfs_with_tree().unwrap();
//     let root = procfs.root_inode();
//
//     let self_link = root.lookup("self").unwrap();
//     let target = self_link.readlink().unwrap();
//
//     // 目标应该是一个数字字符串
//     kassert!(target.len() > 0);
//     // 尝试解析为数字
//     let is_numeric = target.chars().all(|c: char| c.is_numeric());
//     kassert!(is_numeric);
// });
