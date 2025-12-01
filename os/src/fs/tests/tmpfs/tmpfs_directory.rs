//! Tmpfs 目录操作测试

use super::*;
use crate::{kassert, test_case};
use alloc::vec;
use alloc::vec::Vec;

test_case!(test_tmpfs_nested_directories, {
    let fs = create_test_tmpfs();
    let root = fs.root_inode();

    // 创建嵌套目录
    let dir1 = root
        .mkdir("dir1", FileMode::from_bits_truncate(0o755))
        .unwrap();
    let dir2 = dir1
        .mkdir("dir2", FileMode::from_bits_truncate(0o755))
        .unwrap();
    let dir3 = dir2
        .mkdir("dir3", FileMode::from_bits_truncate(0o755))
        .unwrap();

    // 在最深层目录创建文件
    let file = dir3
        .create("deep.txt", FileMode::from_bits_truncate(0o644))
        .unwrap();
    file.write_at(0, b"deep file").unwrap();

    // 验证可以通过路径访问
    let found = root.lookup("dir1").unwrap();
    let found = found.lookup("dir2").unwrap();
    let found = found.lookup("dir3").unwrap();
    let found = found.lookup("deep.txt").unwrap();

    let mut buf = vec![0u8; 9];
    found.read_at(0, &mut buf).unwrap();
    kassert!(&buf[..] == b"deep file");
});

test_case!(test_tmpfs_readdir_nested, {
    let fs = create_test_tmpfs();
    let root = fs.root_inode();

    let dir = root
        .mkdir("parent", FileMode::from_bits_truncate(0o755))
        .unwrap();
    dir.create("file1.txt", FileMode::from_bits_truncate(0o644))
        .unwrap();
    dir.mkdir("subdir", FileMode::from_bits_truncate(0o755))
        .unwrap();
    dir.create("file2.txt", FileMode::from_bits_truncate(0o644))
        .unwrap();

    let entries = dir.readdir().unwrap();
    kassert!(entries.len() == 5); // ., .., file1.txt, subdir, file2.txt

    let names: Vec<_> = entries.iter().map(|e| e.name.as_str()).collect();
    kassert!(names.contains(&"file1.txt"));
    kassert!(names.contains(&"file2.txt"));
    kassert!(names.contains(&"subdir"));
});

test_case!(test_tmpfs_empty_directory, {
    let fs = create_test_tmpfs();
    let root = fs.root_inode();

    let dir = root
        .mkdir("empty", FileMode::from_bits_truncate(0o755))
        .unwrap();

    // 空目录应该只有 . 和 ..
    let entries = dir.readdir().unwrap();
    kassert!(entries.len() == 2);

    let names: Vec<_> = entries.iter().map(|e| e.name.as_str()).collect();
    kassert!(names.contains(&"."));
    kassert!(names.contains(&".."));
});

test_case!(test_tmpfs_directory_not_empty, {
    let fs = create_test_tmpfs();
    let root = fs.root_inode();

    let dir = root
        .mkdir("nonempty", FileMode::from_bits_truncate(0o755))
        .unwrap();
    dir.create("file.txt", FileMode::from_bits_truncate(0o644))
        .unwrap();

    // 尝试删除非空目录应该失败
    let result = root.rmdir("nonempty");
    kassert!(result.is_err());
    kassert!(matches!(result, Err(FsError::DirectoryNotEmpty)));
});

test_case!(test_tmpfs_parent_link, {
    let fs = create_test_tmpfs();
    let root = fs.root_inode();

    let dir1 = root
        .mkdir("dir1", FileMode::from_bits_truncate(0o755))
        .unwrap();
    let dir2 = dir1
        .mkdir("dir2", FileMode::from_bits_truncate(0o755))
        .unwrap();

    // 通过 .. 返回父目录
    let parent = dir2.lookup("..").unwrap();
    kassert!(parent.metadata().unwrap().inode_no == dir1.metadata().unwrap().inode_no);

    // 再往上一层
    let grandparent = parent.lookup("..").unwrap();
    kassert!(grandparent.metadata().unwrap().inode_no == root.metadata().unwrap().inode_no);
});

test_case!(test_tmpfs_many_entries, {
    let fs = create_test_tmpfs();
    let root = fs.root_inode();

    // 创建很多文件
    for i in 0..100 {
        let name = alloc::format!("file_{}.txt", i);
        root.create(&name, FileMode::from_bits_truncate(0o644))
            .unwrap();
    }

    // 验证所有文件都能找到
    let entries = root.readdir().unwrap();
    kassert!(entries.len() == 102); // 100 files + . + ..

    // 随机查找几个
    kassert!(root.lookup("file_0.txt").is_ok());
    kassert!(root.lookup("file_50.txt").is_ok());
    kassert!(root.lookup("file_99.txt").is_ok());
});
