use super::*;
use crate::vfs::file_system::FileSystem;
use crate::{kassert, test_case};
use alloc::vec;

// P1 重要功能测试 - 集成测试

test_case!(test_simplefs_vfs_integration_basic, {
    // 创建 SimpleFS
    let fs = create_test_simplefs();
    let root = fs.root_inode();

    // 通过 VFS 接口创建文件
    let inode = root
        .create("test.txt", FileMode::from_bits_truncate(0o644))
        .unwrap();

    // 写入数据
    let content = b"Integration test";
    inode.write_at(0, content).unwrap();

    // 通过查找验证
    let found = root.lookup("test.txt").unwrap();
    let mut buf = vec![0u8; content.len()];
    found.read_at(0, &mut buf).unwrap();
    kassert!(&buf[..] == content);
});

test_case!(test_simplefs_vfs_integration_directory, {
    // 创建 SimpleFS
    let fs = create_test_simplefs();
    let root = fs.root_inode();

    // 创建目录结构
    let dir1 = root
        .mkdir("dir1", FileMode::from_bits_truncate(0o755))
        .unwrap();
    let dir2 = dir1
        .mkdir("dir2", FileMode::from_bits_truncate(0o755))
        .unwrap();

    // 在嵌套目录中创建文件
    let inode = dir2
        .create("file.txt", FileMode::from_bits_truncate(0o644))
        .unwrap();
    inode.write_at(0, b"nested").unwrap();

    // 验证可以通过路径访问
    let found_dir2 = dir1.lookup("dir2").unwrap();
    let found_file = found_dir2.lookup("file.txt").unwrap();
    let mut buf = vec![0u8; 6];
    found_file.read_at(0, &mut buf).unwrap();
    kassert!(&buf[..] == b"nested");
});

test_case!(test_simplefs_statfs, {
    // 创建 SimpleFS
    let fs = create_test_simplefs();

    // 获取文件系统统计信息
    let statfs = fs.statfs().unwrap();
    kassert!(statfs.block_size > 0);
    kassert!(statfs.max_filename_len > 0);
});

test_case!(test_simplefs_fs_type, {
    // 创建 SimpleFS
    let fs = create_test_simplefs();

    // 验证文件系统类型
    let fs_type = fs.fs_type();
    kassert!(fs_type == "simplefs");
});

test_case!(test_simplefs_sync, {
    // 创建 SimpleFS 并写入数据
    let fs = create_test_simplefs();
    create_test_file_with_content(&fs, "test.txt", b"test").unwrap();

    // 同步文件系统（内存文件系统应该总是成功）
    let result = fs.sync();
    kassert!(result.is_ok());
});

test_case!(test_simplefs_multiple_operations, {
    // 创建 SimpleFS
    let fs = create_test_simplefs();
    let root = fs.root_inode();

    // 执行一系列操作
    // 1. 创建目录
    let dir = root
        .mkdir("testdir", FileMode::from_bits_truncate(0o755))
        .unwrap();

    // 2. 在目录中创建多个文件
    for i in 0..3 {
        let filename = alloc::format!("file{}.txt", i);
        let inode = dir
            .create(&filename, FileMode::from_bits_truncate(0o644))
            .unwrap();
        let content = alloc::format!("Content {}", i);
        inode.write_at(0, content.as_bytes()).unwrap();
    }

    // 3. 列出目录内容
    let entries = dir.readdir().unwrap();
    kassert!(entries.len() == 3);

    // 4. 读取每个文件并验证内容
    for i in 0..3 {
        let filename = alloc::format!("file{}.txt", i);
        let inode = dir.lookup(&filename).unwrap();
        let mut buf = vec![0u8; 10];
        let bytes_read = inode.read_at(0, &mut buf).unwrap();
        let expected = alloc::format!("Content {}", i);
        kassert!(&buf[..bytes_read] == expected.as_bytes());
    }

    // 5. 删除一个文件
    dir.unlink("file1.txt").unwrap();

    // 6. 验证文件被删除
    let result = dir.lookup("file1.txt");
    kassert!(result.is_err());

    // 7. 验证其他文件仍然存在
    let result = dir.lookup("file0.txt");
    kassert!(result.is_ok());
    let result = dir.lookup("file2.txt");
    kassert!(result.is_ok());
});
