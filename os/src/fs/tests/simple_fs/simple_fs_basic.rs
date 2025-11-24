use super::*;
use crate::vfs::file_system::FileSystem;
use crate::{kassert, test_case};
use alloc::vec;

// P0 核心功能测试

test_case!(test_simplefs_create_file, {
    // 创建文件系统
    let fs = create_test_simplefs();
    let root = fs.root_inode();

    // 创建文件
    let result = root.create("test.txt", FileMode::from_bits_truncate(0o644));
    kassert!(result.is_ok());

    // 验证文件存在
    let lookup_result = root.lookup("test.txt");
    kassert!(lookup_result.is_ok());
});

test_case!(test_simplefs_write_and_read, {
    // 创建文件系统和文件
    let fs = create_test_simplefs();
    let content = b"Hello, SimpleFS!";
    let inode = create_test_file_with_content(&fs, "test.txt", content).unwrap();

    // 读取内容
    let mut buf = vec![0u8; content.len()];
    let bytes_read = inode.read_at(0, &mut buf).unwrap();
    kassert!(bytes_read == content.len());
    kassert!(&buf[..] == content);
});

test_case!(test_simplefs_truncate, {
    // 创建文件并写入内容
    let fs = create_test_simplefs();
    let inode = create_test_file_with_content(&fs, "test.txt", b"Hello, World!").unwrap();

    // 截断到 5 字节
    let result = inode.truncate(5);
    kassert!(result.is_ok());

    // 验证大小
    let metadata = inode.metadata().unwrap();
    kassert!(metadata.size == 5);

    // 读取内容
    let mut buf = vec![0u8; 5];
    inode.read_at(0, &mut buf).unwrap();
    kassert!(&buf[..] == b"Hello");
});

test_case!(test_simplefs_unlink_file, {
    // 创建文件系统和文件
    let fs = create_test_simplefs();
    let root = fs.root_inode();
    create_test_file_with_content(&fs, "test.txt", b"test").unwrap();

    // 删除文件
    let result = root.unlink("test.txt");
    kassert!(result.is_ok());

    // 验证文件不存在
    let lookup_result = root.lookup("test.txt");
    kassert!(lookup_result.is_err());
    kassert!(matches!(lookup_result, Err(FsError::NotFound)));
});

// P1 重要功能测试

test_case!(test_simplefs_write_at_offset, {
    // 创建文件并写入初始内容
    let fs = create_test_simplefs();
    let inode = create_test_file_with_content(&fs, "test.txt", b"0123456789").unwrap();

    // 在偏移 5 处写入
    let bytes_written = inode.write_at(5, b"ABCDE").unwrap();
    kassert!(bytes_written == 5);

    // 读取验证
    let mut buf = vec![0u8; 10];
    inode.read_at(0, &mut buf).unwrap();
    kassert!(&buf[..] == b"01234ABCDE");
});

test_case!(test_simplefs_read_at_offset, {
    // 创建文件
    let fs = create_test_simplefs();
    let inode = create_test_file_with_content(&fs, "test.txt", b"0123456789").unwrap();

    // 从偏移 3 读取 5 字节
    let mut buf = vec![0u8; 5];
    let bytes_read = inode.read_at(3, &mut buf).unwrap();
    kassert!(bytes_read == 5);
    kassert!(&buf[..] == b"34567");
});

test_case!(test_simplefs_multiple_files, {
    // 创建多个文件
    let fs = create_test_simplefs();
    let root = fs.root_inode();

    for i in 0..5 {
        let filename = alloc::format!("file{}.txt", i);
        let result = root.create(&filename, FileMode::from_bits_truncate(0o644));
        kassert!(result.is_ok());
    }

    // 验证所有文件都存在
    for i in 0..5 {
        let filename = alloc::format!("file{}.txt", i);
        let lookup_result = root.lookup(&filename);
        kassert!(lookup_result.is_ok());
    }
});

test_case!(test_simplefs_metadata, {
    // 创建文件
    let fs = create_test_simplefs();
    let content = b"Test content";
    let inode = create_test_file_with_content(&fs, "test.txt", content).unwrap();

    // 获取元数据
    let metadata = inode.metadata().unwrap();
    kassert!(metadata.inode_type == InodeType::File);
    kassert!(metadata.size == content.len());
    kassert!(metadata.mode.can_read());
    kassert!(metadata.mode.can_write());
});

// P2 边界和错误处理测试

test_case!(test_simplefs_create_duplicate, {
    // 创建文件系统
    let fs = create_test_simplefs();
    let root = fs.root_inode();

    // 第一次创建
    root.create("test.txt", FileMode::from_bits_truncate(0o644))
        .unwrap();

    // 第二次创建同名文件应该失败
    let result = root.create("test.txt", FileMode::from_bits_truncate(0o644));
    kassert!(result.is_err());
    kassert!(matches!(result, Err(FsError::AlreadyExists)));
});

test_case!(test_simplefs_lookup_nonexistent, {
    // 创建文件系统
    let fs = create_test_simplefs();
    let root = fs.root_inode();

    // 查找不存在的文件
    let result = root.lookup("nonexistent.txt");
    kassert!(result.is_err());
    kassert!(matches!(result, Err(FsError::NotFound)));
});

test_case!(test_simplefs_unlink_nonexistent, {
    // 创建文件系统
    let fs = create_test_simplefs();
    let root = fs.root_inode();

    // 删除不存在的文件
    let result = root.unlink("nonexistent.txt");
    kassert!(result.is_err());
    kassert!(matches!(result, Err(FsError::NotFound)));
});

test_case!(test_simplefs_read_beyond_eof, {
    // 创建小文件
    let fs = create_test_simplefs();
    let inode = create_test_file_with_content(&fs, "test.txt", b"Hello").unwrap();

    // 尝试读取超出文件末尾
    let mut buf = vec![0u8; 10];
    let bytes_read = inode.read_at(0, &mut buf).unwrap();
    kassert!(bytes_read == 5); // 只能读取 5 字节
});

test_case!(test_simplefs_empty_file, {
    // 创建空文件
    let fs = create_test_simplefs();
    let inode = create_test_file_with_content(&fs, "empty.txt", b"").unwrap();

    // 验证元数据
    let metadata = inode.metadata().unwrap();
    kassert!(metadata.size == 0);

    // 读取空文件
    let mut buf = vec![0u8; 10];
    let bytes_read = inode.read_at(0, &mut buf).unwrap();
    kassert!(bytes_read == 0);
});
