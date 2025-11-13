use super::*;
use crate::vfs::file_system::FileSystem;
use crate::{kassert, test_case};
use alloc::vec;
use alloc::vec::Vec;

// P0 核心功能测试

test_case!(test_simplefs_create_directory, {
    // 创建文件系统
    let fs = create_test_simplefs();
    let root = fs.root_inode();

    // 创建目录
    let result = root.mkdir("testdir", FileMode::from_bits_truncate(0o755));
    kassert!(result.is_ok());

    // 验证目录存在
    let lookup_result = root.lookup("testdir");
    kassert!(lookup_result.is_ok());
    let dir_inode = lookup_result.unwrap();
    let metadata = dir_inode.metadata().unwrap();
    kassert!(metadata.inode_type == InodeType::Directory);
});

test_case!(test_simplefs_readdir, {
    // 创建文件系统并添加文件
    let fs = create_test_simplefs();
    let root = fs.root_inode();

    // 创建几个文件
    root.create("file1.txt", FileMode::from_bits_truncate(0o644))
        .unwrap();
    root.create("file2.txt", FileMode::from_bits_truncate(0o644))
        .unwrap();
    root.mkdir("dir1", FileMode::from_bits_truncate(0o755))
        .unwrap();

    // 列出目录内容
    let entries = root.readdir().unwrap();
    kassert!(entries.len() >= 3); // 至少包含我们创建的 3 个项

    // 验证包含我们创建的项
    let names: Vec<_> = entries.iter().map(|e| e.name.as_str()).collect();
    kassert!(names.contains(&"file1.txt"));
    kassert!(names.contains(&"file2.txt"));
    kassert!(names.contains(&"dir1"));
});

test_case!(test_simplefs_nested_directory, {
    // 创建嵌套目录结构
    let fs = create_test_simplefs();
    let root = fs.root_inode();

    // 创建第一级目录
    let dir1 = root
        .mkdir("dir1", FileMode::from_bits_truncate(0o755))
        .unwrap();

    // 在子目录中创建文件
    let result = dir1.create("file.txt", FileMode::from_bits_truncate(0o644));
    kassert!(result.is_ok());

    // 验证文件存在
    let lookup_result = dir1.lookup("file.txt");
    kassert!(lookup_result.is_ok());
});

test_case!(test_simplefs_lookup_in_directory, {
    // 创建文件系统
    let fs = create_test_simplefs();
    let root = fs.root_inode();

    // 创建目录和文件
    let dir = root
        .mkdir("testdir", FileMode::from_bits_truncate(0o755))
        .unwrap();
    dir.create("file.txt", FileMode::from_bits_truncate(0o644))
        .unwrap();

    // 在目录中查找文件
    let result = dir.lookup("file.txt");
    kassert!(result.is_ok());
});

// P1 重要功能测试

test_case!(test_simplefs_unlink_directory, {
    // 创建文件系统和空目录
    let fs = create_test_simplefs();
    let root = fs.root_inode();
    root.mkdir("emptydir", FileMode::from_bits_truncate(0o755))
        .unwrap();

    // 删除空目录
    let result = root.unlink("emptydir");
    kassert!(result.is_ok());

    // 验证目录不存在
    let lookup_result = root.lookup("emptydir");
    kassert!(lookup_result.is_err());
});

test_case!(test_simplefs_readdir_empty, {
    // 创建空目录
    let fs = create_test_simplefs();
    let root = fs.root_inode();
    let dir = root
        .mkdir("emptydir", FileMode::from_bits_truncate(0o755))
        .unwrap();

    // 读取空目录
    let entries = dir.readdir().unwrap();
    kassert!(entries.is_empty() || entries.len() <= 2); // 可能包含 . 和 ..
});

test_case!(test_simplefs_directory_metadata, {
    // 创建目录
    let fs = create_test_simplefs();
    let root = fs.root_inode();
    let dir = root
        .mkdir("testdir", FileMode::from_bits_truncate(0o755))
        .unwrap();

    // 获取元数据
    let metadata = dir.metadata().unwrap();
    kassert!(metadata.inode_type == InodeType::Directory);
    kassert!(metadata.mode.can_read());
    kassert!(metadata.mode.can_write());
    kassert!(metadata.mode.can_execute()); // 目录需要执行权限才能进入
});

// P2 边界和错误处理测试

test_case!(test_simplefs_mkdir_duplicate, {
    // 创建文件系统
    let fs = create_test_simplefs();
    let root = fs.root_inode();

    // 第一次创建
    root.mkdir("testdir", FileMode::from_bits_truncate(0o755))
        .unwrap();

    // 第二次创建同名目录应该失败
    let result = root.mkdir("testdir", FileMode::from_bits_truncate(0o755));
    kassert!(result.is_err());
    kassert!(matches!(result, Err(FsError::AlreadyExists)));
});

test_case!(test_simplefs_write_to_directory, {
    // 创建目录
    let fs = create_test_simplefs();
    let root = fs.root_inode();
    let dir = root
        .mkdir("testdir", FileMode::from_bits_truncate(0o755))
        .unwrap();

    // 尝试写入目录（应该失败）
    let result = dir.write_at(0, b"test");
    kassert!(result.is_err());
    kassert!(matches!(result, Err(FsError::IsDirectory)));
});

test_case!(test_simplefs_read_from_directory, {
    // 创建目录
    let fs = create_test_simplefs();
    let root = fs.root_inode();
    let dir = root
        .mkdir("testdir", FileMode::from_bits_truncate(0o755))
        .unwrap();

    // 尝试读取目录（应该失败）
    let mut buf = vec![0u8; 10];
    let result = dir.read_at(0, &mut buf);
    kassert!(result.is_err());
    kassert!(matches!(result, Err(FsError::IsDirectory)));
});

test_case!(test_simplefs_lookup_in_file, {
    // 创建文件
    let fs = create_test_simplefs();
    let inode = create_test_file_with_content(&fs, "file.txt", b"test").unwrap();

    // 尝试在文件中查找（应该失败）
    let result = inode.lookup("anything");
    kassert!(result.is_err());
    kassert!(matches!(result, Err(FsError::NotDirectory)));
});
