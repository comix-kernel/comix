use super::*;
use crate::vfs::file_system::FileSystem;
use crate::{kassert, test_case};
use alloc::vec;
use alloc::vec::Vec;

// P1 重要功能测试

test_case!(test_ext4_read_at_offset, {
    // 创建文件
    let fs = create_test_ext4();
    let inode = create_test_file_with_content(&fs, "test.txt", b"0123456789").unwrap();

    // 从偏移 3 读取 5 字节
    let mut buf = vec![0u8; 5];
    let bytes_read = inode.read_at(3, &mut buf).unwrap();
    kassert!(bytes_read == 5);
    kassert!(&buf[..] == b"34567");
});

test_case!(test_ext4_write_at_offset, {
    // 创建文件并写入初始内容
    let fs = create_test_ext4();
    let inode = create_test_file_with_content(&fs, "test.txt", b"0123456789").unwrap();

    // 在偏移 5 处写入
    let bytes_written = inode.write_at(5, b"ABCDE").unwrap();
    kassert!(bytes_written == 5);

    // 读取验证
    let mut buf = vec![0u8; 10];
    inode.read_at(0, &mut buf).unwrap();
    kassert!(&buf[..] == b"01234ABCDE");
});

test_case!(test_ext4_overwrite, {
    // 创建文件并写入初始内容
    let fs = create_test_ext4();
    let inode = create_test_file_with_content(&fs, "test.txt", b"Hello").unwrap();

    // 覆盖写入
    inode.write_at(0, b"World").unwrap();

    // 读取验证
    let mut buf = vec![0u8; 5];
    inode.read_at(0, &mut buf).unwrap();
    kassert!(&buf[..] == b"World");
});

test_case!(test_ext4_append_write, {
    // 创建文件并写入初始内容
    let fs = create_test_ext4();
    let inode = create_test_file_with_content(&fs, "test.txt", b"Hello").unwrap();

    // 追加写入
    inode.write_at(5, b" World").unwrap();

    // 读取验证
    let mut buf = vec![0u8; 11];
    inode.read_at(0, &mut buf).unwrap();
    kassert!(&buf[..] == b"Hello World");
});

test_case!(test_ext4_large_data_block, {
    // 创建大数据块 (4KB)
    let fs = create_test_ext4();
    let large_data = alloc::vec![0x42u8; 4096];
    let inode = create_test_file_with_content(&fs, "large.txt", &large_data).unwrap();

    // 验证大小
    let metadata = inode.metadata().unwrap();
    kassert!(metadata.size == 4096);

    // 读取部分验证
    let mut buf = vec![0u8; 100];
    inode.read_at(0, &mut buf).unwrap();
    kassert!(buf[0] == 0x42);
    kassert!(buf[99] == 0x42);
});

test_case!(test_ext4_multiple_writes, {
    // 创建文件
    let fs = create_test_ext4();
    let root = fs.root_inode();
    let inode = root
        .create("test.txt", FileMode::from_bits_truncate(0o644))
        .unwrap();

    // 多次写入
    inode.write_at(0, b"AAA").unwrap();
    inode.write_at(3, b"BBB").unwrap();
    inode.write_at(6, b"CCC").unwrap();

    // 读取验证
    let mut buf = vec![0u8; 9];
    inode.read_at(0, &mut buf).unwrap();
    kassert!(&buf[..] == b"AAABBBCCC");
});

test_case!(test_ext4_sync, {
    // 创建文件并写入
    let fs = create_test_ext4();
    let inode = create_test_file_with_content(&fs, "test.txt", b"test data").unwrap();

    // 同步文件
    let result = inode.sync();
    kassert!(result.is_ok());

    // 同步文件系统
    let result = fs.sync();
    kassert!(result.is_ok());
});

// P2 边界和错误处理测试

test_case!(test_ext4_read_beyond_eof, {
    // 创建小文件
    let fs = create_test_ext4();
    let inode = create_test_file_with_content(&fs, "test.txt", b"Hello").unwrap();

    // 尝试读取超出文件末尾
    let mut buf = vec![0u8; 10];
    let bytes_read = inode.read_at(0, &mut buf).unwrap();
    kassert!(bytes_read <= 5); // 最多只能读取 5 字节
});

test_case!(test_ext4_empty_file_read, {
    // 创建空文件
    let fs = create_test_ext4();
    let inode = create_test_file_with_content(&fs, "empty.txt", b"").unwrap();

    // 读取空文件
    let mut buf = vec![0u8; 10];
    let bytes_read = inode.read_at(0, &mut buf).unwrap();
    kassert!(bytes_read == 0);
});

test_case!(test_ext4_truncate_to_zero, {
    // 创建文件并写入内容
    let fs = create_test_ext4();
    let inode = create_test_file_with_content(&fs, "test.txt", b"Hello, World!").unwrap();

    // 截断到 0 字节
    let result = inode.truncate(0);
    kassert!(result.is_ok());

    // 验证大小
    let metadata = inode.metadata().unwrap();
    kassert!(metadata.size == 0);
});

test_case!(test_ext4_truncate_extend, {
    // 创建文件
    let fs = create_test_ext4();
    let inode = create_test_file_with_content(&fs, "test.txt", b"Hello").unwrap();

    // 截断扩展到 20 字节
    let result = inode.truncate(20);
    kassert!(result.is_ok());

    // 验证大小
    let metadata = inode.metadata().unwrap();
    kassert!(metadata.size == 20);
});
