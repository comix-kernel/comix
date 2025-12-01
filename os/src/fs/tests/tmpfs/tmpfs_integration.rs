//! Tmpfs 集成测试 - 综合场景

use super::*;
use crate::{kassert, test_case};
use alloc::vec;

test_case!(test_tmpfs_complex_scenario, {
    let fs = create_test_tmpfs();
    let root = fs.root_inode();

    // 创建复杂的目录结构
    let etc = root
        .mkdir("etc", FileMode::from_bits_truncate(0o755))
        .unwrap();
    let home = root
        .mkdir("home", FileMode::from_bits_truncate(0o755))
        .unwrap();
    let tmp = root
        .mkdir("tmp", FileMode::from_bits_truncate(0o755))
        .unwrap();

    // 在 /etc 中创建配置文件
    let passwd = etc
        .create("passwd", FileMode::from_bits_truncate(0o644))
        .unwrap();
    passwd.write_at(0, b"root:x:0:0::/root:/bin/sh\n").unwrap();

    // 在 /home 中创建用户目录
    let user = home
        .mkdir("user", FileMode::from_bits_truncate(0o755))
        .unwrap();
    let bashrc = user
        .create(".bashrc", FileMode::from_bits_truncate(0o644))
        .unwrap();
    bashrc.write_at(0, b"export PATH=/usr/bin\n").unwrap();

    // 在 /tmp 中创建临时文件
    let temp = tmp
        .create("temp.txt", FileMode::from_bits_truncate(0o644))
        .unwrap();
    temp.write_at(0, b"temporary data").unwrap();

    // 验证所有文件都存在并可读
    let found = root.lookup("etc").unwrap().lookup("passwd").unwrap();
    let mut buf = vec![0u8; 26];
    found.read_at(0, &mut buf).unwrap();
    kassert!(&buf[..] == b"root:x:0:0::/root:/bin/sh\n");
});

test_case!(test_tmpfs_concurrent_operations, {
    let fs = create_test_tmpfs();
    let root = fs.root_inode();

    // 创建多个文件并同时操作
    let file1 = root
        .create("file1.txt", FileMode::from_bits_truncate(0o644))
        .unwrap();
    let file2 = root
        .create("file2.txt", FileMode::from_bits_truncate(0o644))
        .unwrap();
    let file3 = root
        .create("file3.txt", FileMode::from_bits_truncate(0o644))
        .unwrap();

    // 同时写入
    file1.write_at(0, b"File 1 content").unwrap();
    file2.write_at(0, b"File 2 content").unwrap();
    file3.write_at(0, b"File 3 content").unwrap();

    // 验证数据独立
    let mut buf = vec![0u8; 14];
    file1.read_at(0, &mut buf).unwrap();
    kassert!(&buf[..] == b"File 1 content");

    file2.read_at(0, &mut buf).unwrap();
    kassert!(&buf[..] == b"File 2 content");

    file3.read_at(0, &mut buf).unwrap();
    kassert!(&buf[..] == b"File 3 content");
});

test_case!(test_tmpfs_lifecycle, {
    let fs = create_test_tmpfs();
    let root = fs.root_inode();

    // 创建文件
    let file = root
        .create("lifecycle.txt", FileMode::from_bits_truncate(0o644))
        .unwrap();

    // 写入数据
    file.write_at(0, b"initial data").unwrap();

    // 覆盖数据
    file.write_at(0, b"updated data").unwrap();

    // 扩展数据
    file.write_at(12, b" appended").unwrap();

    // 截断
    file.truncate(7).unwrap();

    // 验证最终状态
    let mut buf = vec![0u8; 7];
    file.read_at(0, &mut buf).unwrap();
    kassert!(&buf[..] == b"updated");

    // 删除
    root.unlink("lifecycle.txt").unwrap();
    kassert!(root.lookup("lifecycle.txt").is_err());
});

test_case!(test_tmpfs_stats, {
    let fs = create_test_tmpfs();
    let root = fs.root_inode();

    // 检查初始状态
    let initial_size = fs.used_size();

    // 创建一些文件
    for i in 0..10 {
        let name = alloc::format!("file_{}.txt", i);
        let file = root
            .create(&name, FileMode::from_bits_truncate(0o644))
            .unwrap();
        file.write_at(0, &[0xAB; 8192]).unwrap(); // 2 pages per file
    }

    // 检查使用量增加
    let used_size = fs.used_size();
    kassert!(used_size > initial_size);
    kassert!(used_size >= 10 * 8192); // 至少 10 个文件的数据
});

test_case!(test_tmpfs_metadata_update, {
    let fs = create_test_tmpfs();
    let file = create_test_file_with_content(&fs, "test.txt", b"hello").unwrap();

    let meta1 = file.metadata().unwrap();
    kassert!(meta1.size == 5);

    // 写入更多数据
    file.write_at(5, b", world!").unwrap();

    let meta2 = file.metadata().unwrap();
    kassert!(meta2.size == 13);

    // mtime 应该更新
    kassert!(meta2.mtime.tv_sec >= meta1.mtime.tv_sec);
});

test_case!(test_tmpfs_shared_access, {
    let fs = create_test_tmpfs();
    let root = fs.root_inode();

    // 创建文件
    let file1 = root
        .create("shared.txt", FileMode::from_bits_truncate(0o644))
        .unwrap();

    // 通过 lookup 获取同一文件
    let file2 = root.lookup("shared.txt").unwrap();

    // 通过 file1 写入
    file1.write_at(0, b"shared data").unwrap();

    // 通过 file2 读取
    let mut buf = vec![0u8; 11];
    file2.read_at(0, &mut buf).unwrap();
    kassert!(&buf[..] == b"shared data");
});
