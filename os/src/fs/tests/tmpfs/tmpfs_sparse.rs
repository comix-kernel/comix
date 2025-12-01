//! Tmpfs 稀疏文件测试

use super::*;
use crate::{kassert, test_case};
use alloc::vec;

test_case!(test_tmpfs_sparse_hole, {
    // 创建文件并在中间留空洞
    let fs = create_test_tmpfs();
    let root = fs.root_inode();
    let file = root
        .create("sparse.dat", FileMode::from_bits_truncate(0o644))
        .unwrap();

    // 在偏移 0 写入数据
    let data1 = b"START";
    kassert!(file.write_at(0, data1).unwrap() == data1.len());

    // 在偏移 1MB 写入数据（中间有空洞）
    let data2 = b"END";
    let offset = 1024 * 1024;
    kassert!(file.write_at(offset, data2).unwrap() == data2.len());

    // 验证文件大小
    let metadata = file.metadata().unwrap();
    kassert!(metadata.size == offset + data2.len());

    // 读取开始部分
    let mut buf = vec![0u8; 5];
    kassert!(file.read_at(0, &mut buf).unwrap() == 5);
    kassert!(&buf == data1);

    // 读取空洞部分（应该返回0）
    let mut hole_buf = vec![0xFF; 1024];
    let read = file.read_at(1024, &mut hole_buf).unwrap();
    kassert!(read == 1024);
    // 空洞应该被填充为0
    for byte in &hole_buf {
        kassert!(*byte == 0);
    }

    // 读取结束部分
    let mut buf2 = vec![0u8; 3];
    kassert!(file.read_at(offset, &mut buf2).unwrap() == 3);
    kassert!(&buf2 == data2);
});

test_case!(test_tmpfs_sparse_multiple_holes, {
    let fs = create_test_tmpfs();
    let root = fs.root_inode();
    let file = root
        .create("multi_sparse.dat", FileMode::from_bits_truncate(0o644))
        .unwrap();

    // 在多个位置写入数据
    let data = b"BLOCK";
    kassert!(file.write_at(0, data).is_ok());
    kassert!(file.write_at(4096, data).is_ok());
    kassert!(file.write_at(8192, data).is_ok());
    kassert!(file.write_at(16384, data).is_ok());

    // 验证文件大小
    let metadata = file.metadata().unwrap();
    kassert!(metadata.size == 16384 + data.len());

    // 验证各个数据块
    let mut buf = vec![0u8; 5];
    kassert!(file.read_at(0, &mut buf).unwrap() == 5);
    kassert!(&buf == data);

    kassert!(file.read_at(4096, &mut buf).unwrap() == 5);
    kassert!(&buf == data);

    kassert!(file.read_at(8192, &mut buf).unwrap() == 5);
    kassert!(&buf == data);

    kassert!(file.read_at(16384, &mut buf).unwrap() == 5);
    kassert!(&buf == data);
});

test_case!(test_tmpfs_sparse_truncate_extend, {
    let fs = create_test_tmpfs();
    let root = fs.root_inode();
    let file = root
        .create("truncate.dat", FileMode::from_bits_truncate(0o644))
        .unwrap();

    // 写入初始数据
    let data = b"INITIAL";
    kassert!(file.write_at(0, data).is_ok());

    // 扩展文件（创建空洞）
    kassert!(file.truncate(1024 * 1024).is_ok());

    // 验证文件大小
    let metadata = file.metadata().unwrap();
    kassert!(metadata.size == 1024 * 1024);

    // 读取初始数据
    let mut buf = vec![0u8; 7];
    kassert!(file.read_at(0, &mut buf).unwrap() == 7);
    kassert!(&buf == data);

    // 读取扩展部分（应该是0）
    let mut hole = vec![0xFF; 1024];
    kassert!(file.read_at(1024, &mut hole).unwrap() == 1024);
    for byte in &hole {
        kassert!(*byte == 0);
    }
});

test_case!(test_tmpfs_sparse_write_beyond_eof, {
    let fs = create_test_tmpfs();
    let root = fs.root_inode();
    let file = root
        .create("beyond.dat", FileMode::from_bits_truncate(0o644))
        .unwrap();

    // 直接在文件末尾之外写入
    let data = b"BEYOND";
    let offset = 1024 * 1024;
    kassert!(file.write_at(offset, data).is_ok());

    // 验证文件大小
    let metadata = file.metadata().unwrap();
    kassert!(metadata.size == offset + data.len());

    // 读取前面的空洞
    let mut hole = vec![0xFF; 1024];
    kassert!(file.read_at(0, &mut hole).unwrap() == 1024);
    for byte in &hole {
        kassert!(*byte == 0);
    }

    // 读取实际数据
    let mut buf = vec![0u8; 6];
    kassert!(file.read_at(offset, &mut buf).unwrap() == 6);
    kassert!(&buf == data);
});

test_case!(test_tmpfs_sparse_fill_hole, {
    let fs = create_test_tmpfs();
    let root = fs.root_inode();
    let file = root
        .create("fill.dat", FileMode::from_bits_truncate(0o644))
        .unwrap();

    // 创建空洞
    kassert!(file.write_at(0, b"START").is_ok());
    kassert!(file.write_at(8192, b"END").is_ok());

    // 填充空洞
    let fill_data = vec![0xAA; 4096];
    kassert!(file.write_at(2048, &fill_data).is_ok());

    // 验证填充的数据
    let mut buf = vec![0u8; 4096];
    kassert!(file.read_at(2048, &mut buf).unwrap() == 4096);
    kassert!(buf == fill_data);
});

test_case!(test_tmpfs_sparse_empty_truncate, {
    let fs = create_test_tmpfs();
    let root = fs.root_inode();
    let file = root
        .create("empty_trunc.dat", FileMode::from_bits_truncate(0o644))
        .unwrap();

    // 不写入任何数据，直接扩展
    kassert!(file.truncate(1024 * 1024).is_ok());

    // 验证文件大小
    let metadata = file.metadata().unwrap();
    kassert!(metadata.size == 1024 * 1024);

    // 读取应该返回全0
    let mut buf = vec![0xFF; 4096];
    kassert!(file.read_at(0, &mut buf).unwrap() == 4096);
    for byte in &buf {
        kassert!(*byte == 0);
    }
});
