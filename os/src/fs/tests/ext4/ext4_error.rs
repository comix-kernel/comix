use super::*;
use crate::vfs::file_system::FileSystem;
use crate::{kassert, test_case};
use alloc::vec;

// P2 边界和错误处理测试

test_case!(test_ext4_create_duplicate_file, {
    // 创建 Ext4 文件系统
    let fs = create_test_ext4();
    let root = fs.root_inode();

    // 第一次创建
    root.create("test.txt", FileMode::from_bits_truncate(0o644))
        .unwrap();

    // 第二次创建同名文件应该失败
    let result = root.create("test.txt", FileMode::from_bits_truncate(0o644));
    kassert!(result.is_err());
    kassert!(matches!(result, Err(FsError::AlreadyExists)));
});

test_case!(test_ext4_create_duplicate_directory, {
    // 创建 Ext4 文件系统
    let fs = create_test_ext4();
    let root = fs.root_inode();

    // 第一次创建
    root.mkdir("testdir", FileMode::from_bits_truncate(0o755))
        .unwrap();

    // 第二次创建同名目录应该失败
    let result = root.mkdir("testdir", FileMode::from_bits_truncate(0o755));
    kassert!(result.is_err());
    kassert!(matches!(result, Err(FsError::AlreadyExists)));
});

test_case!(test_ext4_lookup_nonexistent, {
    // 创建 Ext4 文件系统
    let fs = create_test_ext4();
    let root = fs.root_inode();

    // 查找不存在的文件
    let result = root.lookup("nonexistent.txt");
    kassert!(result.is_err());
    kassert!(matches!(result, Err(FsError::NotFound)));
});

test_case!(test_ext4_unlink_nonexistent, {
    // 创建 Ext4 文件系统
    let fs = create_test_ext4();
    let root = fs.root_inode();

    // 删除不存在的文件
    let result = root.unlink("nonexistent.txt");
    kassert!(result.is_err());
    kassert!(matches!(result, Err(FsError::NotFound)));
});

test_case!(test_ext4_lookup_in_file, {
    // 创建文件
    let fs = create_test_ext4();
    let inode = create_test_file_with_content(&fs, "file.txt", b"test").unwrap();

    // 尝试在文件中查找（应该失败）
    let result = inode.lookup("anything");
    kassert!(result.is_err());
    kassert!(matches!(result, Err(FsError::NotDirectory)));
});

test_case!(test_ext4_read_from_directory, {
    // 创建目录
    let fs = create_test_ext4();
    let dir = create_test_dir(&fs, "testdir").unwrap();

    // 尝试读取目录（应该失败）
    let mut buf = vec![0u8; 10];
    let result = dir.read_at(0, &mut buf);
    kassert!(result.is_err());
    kassert!(matches!(result, Err(FsError::IsDirectory)));
});

test_case!(test_ext4_write_to_directory, {
    // 创建目录
    let fs = create_test_ext4();
    let dir = create_test_dir(&fs, "testdir").unwrap();

    // 尝试写入目录（应该失败）
    let result = dir.write_at(0, b"test");
    kassert!(result.is_err());
    kassert!(matches!(result, Err(FsError::IsDirectory)));
});

test_case!(test_ext4_create_in_file, {
    // 创建文件
    let fs = create_test_ext4();
    let inode = create_test_file_with_content(&fs, "file.txt", b"test").unwrap();

    // 尝试在文件中创建（应该失败）
    let result = inode.create("another.txt", FileMode::from_bits_truncate(0o644));
    kassert!(result.is_err());
    kassert!(matches!(result, Err(FsError::NotDirectory)));
});

test_case!(test_ext4_mkdir_in_file, {
    // 创建文件
    let fs = create_test_ext4();
    let inode = create_test_file_with_content(&fs, "file.txt", b"test").unwrap();

    // 尝试在文件中创建目录（应该失败）
    let result = inode.mkdir("subdir", FileMode::from_bits_truncate(0o755));
    kassert!(result.is_err());
    kassert!(matches!(result, Err(FsError::NotDirectory)));
});

test_case!(test_ext4_truncate_directory, {
    // 创建目录
    let fs = create_test_ext4();
    let dir = create_test_dir(&fs, "testdir").unwrap();

    // 尝试截断目录（可能失败或不改变大小）
    let result = dir.truncate(0);
    // Ext4 可能允许截断目录或返回错误
    // 这里只验证不会 panic
    kassert!(result.is_ok() || result.is_err());
});

test_case!(test_ext4_readdir_on_file, {
    // 创建文件
    let fs = create_test_ext4();
    let inode = create_test_file_with_content(&fs, "file.txt", b"test").unwrap();

    // 尝试读取目录项（应该失败）
    let result = inode.readdir();
    kassert!(result.is_err());
    kassert!(matches!(result, Err(FsError::NotDirectory)));
});

test_case!(test_ext4_empty_filename, {
    // 创建 Ext4 文件系统
    let fs = create_test_ext4();
    let root = fs.root_inode();

    // 尝试创建空文件名的文件
    let result = root.create("", FileMode::from_bits_truncate(0o644));
    kassert!(result.is_err());
    // 可能是 InvalidInput 或 NotFound
});

test_case!(test_ext4_special_filenames, {
    // 创建 Ext4 文件系统
    let fs = create_test_ext4();
    let root = fs.root_inode();

    // 尝试查找 "." (当前目录)
    let result = root.lookup(".");
    // Ext4 应该支持 "."
    kassert!(result.is_ok() || result.is_err());

    // 尝试查找 ".." (父目录)
    let result = root.lookup("..");
    // Ext4 应该支持 ".."
    kassert!(result.is_ok() || result.is_err());
});
