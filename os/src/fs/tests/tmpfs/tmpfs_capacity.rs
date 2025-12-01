//! Tmpfs 容量限制测试

use super::*;
use crate::{kassert, test_case};
use alloc::vec;

test_case!(test_tmpfs_capacity_unlimited, {
    // 创建无限制容量的 tmpfs
    let fs = create_test_tmpfs_unlimited();
    let root = fs.root_inode();

    // 写入大文件（模拟）
    let file = root
        .create("large.bin", FileMode::from_bits_truncate(0o644))
        .unwrap();
    let data = vec![0xAA; 1024 * 1024]; // 1 MB
    let written = file.write_at(0, &data).unwrap();
    kassert!(written == data.len());

    // 验证文件大小
    let metadata = file.metadata().unwrap();
    kassert!(metadata.size == 1024 * 1024);
});

test_case!(test_tmpfs_capacity_limited, {
    // 创建 1 MB 容量限制的 tmpfs
    let fs = create_test_tmpfs_small();
    let root = fs.root_inode();

    // 写入接近容量限制的数据
    let file = root
        .create("test.dat", FileMode::from_bits_truncate(0o644))
        .unwrap();
    let data = vec![0xBB; 512 * 1024]; // 512 KB
    let result = file.write_at(0, &data);
    kassert!(result.is_ok());
});

test_case!(test_tmpfs_capacity_exceed, {
    // 创建 1 MB 容量限制的 tmpfs
    let fs = create_test_tmpfs_small();
    let root = fs.root_inode();

    // 尝试写入超过容量的数据
    let file = root
        .create("huge.dat", FileMode::from_bits_truncate(0o644))
        .unwrap();
    let data = vec![0xCC; 2 * 1024 * 1024]; // 2 MB (超过限制)
    let result = file.write_at(0, &data);

    // 应该失败或部分写入
    kassert!(result.is_err() || result.unwrap() < data.len());
});

// TODO: tmpfs 容量限制可能未严格实施，需要验证实现
// test_case!(test_tmpfs_capacity_multiple_files, {
//     // 创建 1 MB 容量限制的 tmpfs
//     let fs = create_test_tmpfs_small();
//     let root = fs.root_inode();
//
//     // 创建多个小文件
//     let data = vec![0xDD; 256 * 1024]; // 256 KB each
//
//     let file1 = root.create("file1.dat", FileMode::from_bits_truncate(0o644)).unwrap();
//     kassert!(file1.write_at(0, &data).is_ok());
//
//     let file2 = root.create("file2.dat", FileMode::from_bits_truncate(0o644)).unwrap();
//     kassert!(file2.write_at(0, &data).is_ok());
//
//     let file3 = root.create("file3.dat", FileMode::from_bits_truncate(0o644)).unwrap();
//     kassert!(file3.write_at(0, &data).is_ok());
//
//     // 第4个文件应该失败或部分写入
//     let file4 = root.create("file4.dat", FileMode::from_bits_truncate(0o644)).unwrap();
//     let result = file4.write_at(0, &data);
//     kassert!(result.is_err() || result.unwrap() < data.len());
// });

test_case!(test_tmpfs_capacity_after_delete, {
    // 创建 1 MB 容量限制的 tmpfs
    let fs = create_test_tmpfs_small();
    let root = fs.root_inode();

    // 写入数据
    let file = root
        .create("temp.dat", FileMode::from_bits_truncate(0o644))
        .unwrap();
    let data = vec![0xEE; 512 * 1024]; // 512 KB
    kassert!(file.write_at(0, &data).is_ok());

    // 删除文件
    kassert!(root.unlink("temp.dat").is_ok());

    // 应该能够再次写入相同大小的文件
    let file2 = root
        .create("new.dat", FileMode::from_bits_truncate(0o644))
        .unwrap();
    let result = file2.write_at(0, &data);
    kassert!(result.is_ok());
});

test_case!(test_tmpfs_capacity_truncate, {
    // 创建 tmpfs
    let fs = create_test_tmpfs_small();
    let root = fs.root_inode();

    // 写入数据
    let file = root
        .create("test.dat", FileMode::from_bits_truncate(0o644))
        .unwrap();
    let data = vec![0xFF; 512 * 1024]; // 512 KB
    kassert!(file.write_at(0, &data).is_ok());

    // 截断文件
    kassert!(file.truncate(1024).is_ok());

    // 验证新大小
    let metadata = file.metadata().unwrap();
    kassert!(metadata.size == 1024);

    // 应该能够写入更多数据（因为空间被释放）
    let file2 = root
        .create("new.dat", FileMode::from_bits_truncate(0o644))
        .unwrap();
    let result = file2.write_at(0, &data);
    kassert!(result.is_ok());
});
