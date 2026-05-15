//! Tmpfs I/O 操作测试

use super::*;
use crate::{kassert, test_case};
use alloc::vec;

test_case!(test_tmpfs_large_write, {
    let fs = create_test_tmpfs();
    let root = fs.root_inode();

    let file = root
        .create("large.txt", FileMode::from_bits_truncate(0o644))
        .unwrap();

    // 写入 1 MB 数据
    let data = vec![0xAB; 1024 * 1024];
    let written = file.write_at(0, &data).unwrap();
    kassert!(written == data.len());

    // 验证大小
    kassert!(file.metadata().unwrap().size == data.len());
});

test_case!(test_tmpfs_random_access, {
    let fs = create_test_tmpfs();
    let inode = create_test_file_with_content(&fs, "test.txt", b"0123456789").unwrap();

    // 读取中间部分
    let mut buf = vec![0u8; 5];
    let read = inode.read_at(3, &mut buf).unwrap();
    kassert!(read == 5);
    kassert!(&buf[..] == b"34567");

    // 写入中间部分
    inode.write_at(5, b"XXXXX").unwrap();

    // 读取全部
    let mut buf = vec![0u8; 10];
    inode.read_at(0, &mut buf).unwrap();
    kassert!(&buf[..] == b"01234XXXXX");
});

test_case!(test_tmpfs_append, {
    let fs = create_test_tmpfs();
    let inode = create_test_file_with_content(&fs, "test.txt", b"Hello").unwrap();

    // 追加数据
    let offset = inode.metadata().unwrap().size;
    inode.write_at(offset, b", World!").unwrap();

    // 读取全部
    let mut buf = vec![0u8; 13];
    inode.read_at(0, &mut buf).unwrap();
    kassert!(&buf[..] == b"Hello, World!");
});

test_case!(test_tmpfs_overwrite, {
    let fs = create_test_tmpfs();
    let inode = create_test_file_with_content(&fs, "test.txt", b"Hello, World!").unwrap();

    // 覆盖部分数据
    inode.write_at(7, b"Tmpfs").unwrap();

    // 读取
    let mut buf = vec![0u8; 13];
    inode.read_at(0, &mut buf).unwrap();
    kassert!(&buf[..] == b"Hello, Tmpfs!");
});

test_case!(test_tmpfs_read_beyond_end, {
    let fs = create_test_tmpfs();
    let inode = create_test_file_with_content(&fs, "test.txt", b"Hello").unwrap();

    // 从文件末尾之后读取
    let mut buf = vec![0xFFu8; 10];
    let read = inode.read_at(10, &mut buf).unwrap();
    kassert!(read == 0);

    // 从接近末尾读取
    let mut buf = vec![0xFFu8; 10];
    let read = inode.read_at(3, &mut buf).unwrap();
    kassert!(read == 2); // 只读到 "lo"
    kassert!(&buf[..2] == b"lo");
});

test_case!(test_tmpfs_empty_write, {
    let fs = create_test_tmpfs();
    let root = fs.root_inode();

    let file = root
        .create("empty.txt", FileMode::from_bits_truncate(0o644))
        .unwrap();

    // 写入空数据
    let written = file.write_at(0, b"").unwrap();
    kassert!(written == 0);
    kassert!(file.metadata().unwrap().size == 0);
});

test_case!(test_tmpfs_sparse_read, {
    let fs = create_test_tmpfs();
    let root = fs.root_inode();

    let file = root
        .create("sparse.txt", FileMode::from_bits_truncate(0o644))
        .unwrap();

    // 在偏移 8192 处写入
    file.write_at(8192, b"data").unwrap();

    // 读取前面的空洞
    let mut buf = vec![0xFFu8; 100];
    let read = file.read_at(0, &mut buf).unwrap();
    kassert!(read == 100);
    kassert!(buf.iter().all(|&b| b == 0));

    // 读取写入的数据
    let mut buf = vec![0u8; 4];
    file.read_at(8192, &mut buf).unwrap();
    kassert!(&buf[..] == b"data");
});

test_case!(test_tmpfs_cross_page_write, {
    let fs = create_test_tmpfs();
    let root = fs.root_inode();

    let file = root
        .create("cross.txt", FileMode::from_bits_truncate(0o644))
        .unwrap();

    // 跨页写入（4095 处开始，跨越 4096 边界）
    let data = b"CROSS_PAGE_BOUNDARY";
    file.write_at(4095, data).unwrap();

    // 读取验证
    let mut buf = vec![0u8; data.len()];
    file.read_at(4095, &mut buf).unwrap();
    kassert!(&buf[..] == data);
});
