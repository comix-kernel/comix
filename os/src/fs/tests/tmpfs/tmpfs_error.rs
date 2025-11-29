//! Tmpfs 错误处理测试

use super::*;
use crate::{kassert, test_case};

test_case!(test_tmpfs_create_in_file, {
    let fs = create_test_tmpfs();
    let root = fs.root_inode();

    let file = root
        .create("file.txt", FileMode::from_bits_truncate(0o644))
        .unwrap();

    // 尝试在文件中创建文件应该失败
    let result = file.create("invalid", FileMode::from_bits_truncate(0o644));
    kassert!(matches!(result, Err(FsError::NotDirectory)));
});

test_case!(test_tmpfs_mkdir_in_file, {
    let fs = create_test_tmpfs();
    let root = fs.root_inode();

    let file = root
        .create("file.txt", FileMode::from_bits_truncate(0o644))
        .unwrap();

    // 尝试在文件中创建目录应该失败
    let result = file.mkdir("invalid", FileMode::from_bits_truncate(0o755));
    kassert!(matches!(result, Err(FsError::NotDirectory)));
});

test_case!(test_tmpfs_lookup_in_file, {
    let fs = create_test_tmpfs();
    let root = fs.root_inode();

    let file = root
        .create("file.txt", FileMode::from_bits_truncate(0o644))
        .unwrap();

    // 尝试在文件中查找应该失败
    let result = file.lookup("something");
    kassert!(matches!(result, Err(FsError::NotDirectory)));
});

test_case!(test_tmpfs_unlink_directory, {
    let fs = create_test_tmpfs();
    let root = fs.root_inode();

    root.mkdir("dir", FileMode::from_bits_truncate(0o755))
        .unwrap();

    // 尝试用 unlink 删除目录应该失败
    let result = root.unlink("dir");
    kassert!(matches!(result, Err(FsError::IsDirectory)));
});

test_case!(test_tmpfs_rmdir_file, {
    let fs = create_test_tmpfs();
    let root = fs.root_inode();

    root.create("file.txt", FileMode::from_bits_truncate(0o644))
        .unwrap();

    // 尝试用 rmdir 删除文件应该失败
    let result = root.rmdir("file.txt");
    kassert!(matches!(result, Err(FsError::NotDirectory)));
});

test_case!(test_tmpfs_lookup_nonexistent, {
    let fs = create_test_tmpfs();
    let root = fs.root_inode();

    // 查找不存在的文件
    let result = root.lookup("nonexistent.txt");
    kassert!(matches!(result, Err(FsError::NotFound)));
});

test_case!(test_tmpfs_delete_nonexistent, {
    let fs = create_test_tmpfs();
    let root = fs.root_inode();

    // 删除不存在的文件
    let result = root.unlink("nonexistent.txt");
    kassert!(matches!(result, Err(FsError::NotFound)));

    // 删除不存在的目录
    let result = root.rmdir("nonexistent_dir");
    kassert!(matches!(result, Err(FsError::NotFound)));
});

test_case!(test_tmpfs_write_to_directory, {
    let fs = create_test_tmpfs();
    let root = fs.root_inode();

    let dir = root
        .mkdir("dir", FileMode::from_bits_truncate(0o755))
        .unwrap();

    // 尝试向目录写入应该失败
    let result = dir.write_at(0, b"data");
    kassert!(matches!(result, Err(FsError::IsDirectory)));
});

test_case!(test_tmpfs_read_from_directory, {
    let fs = create_test_tmpfs();
    let root = fs.root_inode();

    let dir = root
        .mkdir("dir", FileMode::from_bits_truncate(0o755))
        .unwrap();

    // 尝试从目录读取应该失败
    let mut buf = [0u8; 10];
    let result = dir.read_at(0, &mut buf);
    kassert!(matches!(result, Err(FsError::IsDirectory)));
});

test_case!(test_tmpfs_truncate_directory, {
    let fs = create_test_tmpfs();
    let root = fs.root_inode();

    let dir = root
        .mkdir("dir", FileMode::from_bits_truncate(0o755))
        .unwrap();

    // 尝试截断目录应该失败
    let result = dir.truncate(0);
    kassert!(matches!(result, Err(FsError::IsDirectory)));
});

test_case!(test_tmpfs_capacity_exceeded, {
    let fs = create_test_tmpfs_small(); // 1 MB
    let root = fs.root_inode();

    let file = root
        .create("large.txt", FileMode::from_bits_truncate(0o644))
        .unwrap();

    // 尝试写入超过容量的数据
    let data = alloc::vec![0xAB; 2 * 1024 * 1024]; // 2 MB
    let result = file.write_at(0, &data);

    // 应该失败并返回 NoSpace
    kassert!(matches!(result, Err(FsError::NoSpace)));
});
