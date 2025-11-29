//! Tmpfs 基础功能测试

use super::*;
use crate::{kassert, test_case};
use alloc::vec;
use alloc::vec::Vec;

test_case!(test_tmpfs_basic_create, {
    // 创建 tmpfs
    let fs = create_test_tmpfs();
    let root = fs.root_inode();

    // 创建文件
    let result = root.create("test.txt", FileMode::from_bits_truncate(0o644));
    kassert!(result.is_ok());

    let file = result.unwrap();

    // 验证文件元数据
    let metadata = file.metadata().unwrap();
    kassert!(metadata.inode_type == InodeType::File);
    kassert!(metadata.size == 0);
});

test_case!(test_tmpfs_write_read, {
    let fs = create_test_tmpfs();
    let data = b"Hello, tmpfs!";
    let inode = create_test_file_with_content(&fs, "hello.txt", data).unwrap();

    // 读取数据
    let mut buf = vec![0u8; 32];
    let read = inode.read_at(0, &mut buf).unwrap();
    kassert!(read == data.len());
    kassert!(&buf[..read] == data);

    // 验证文件大小
    let metadata = inode.metadata().unwrap();
    kassert!(metadata.size == data.len());
});

test_case!(test_tmpfs_mkdir, {
    let fs = create_test_tmpfs();
    let root = fs.root_inode();

    // 创建目录
    let result = root.mkdir("testdir", FileMode::from_bits_truncate(0o755));
    kassert!(result.is_ok());

    let dir = result.unwrap();

    // 验证目录元数据
    let metadata = dir.metadata().unwrap();
    kassert!(metadata.inode_type == InodeType::Directory);

    // 在目录中创建文件
    let file = dir
        .create("file_in_dir.txt", FileMode::from_bits_truncate(0o644))
        .unwrap();

    file.write_at(0, b"data").unwrap();

    // 通过 lookup 查找
    let found = dir.lookup("file_in_dir.txt").unwrap();
    let mut buf = vec![0u8; 4];
    found.read_at(0, &mut buf).unwrap();
    kassert!(&buf[..] == b"data");
});

test_case!(test_tmpfs_readdir, {
    let fs = create_test_tmpfs();
    let root = fs.root_inode();

    // 创建多个文件和目录
    root.create("file1.txt", FileMode::from_bits_truncate(0o644))
        .unwrap();
    root.create("file2.txt", FileMode::from_bits_truncate(0o644))
        .unwrap();
    root.mkdir("dir1", FileMode::from_bits_truncate(0o755))
        .unwrap();

    // 读取目录内容
    let entries = root.readdir().unwrap();

    // 应该包含 ".", "..", "file1.txt", "file2.txt", "dir1"
    kassert!(entries.len() == 5);

    let names: Vec<_> = entries.iter().map(|e| e.name.as_str()).collect();
    kassert!(names.contains(&"."));
    kassert!(names.contains(&".."));
    kassert!(names.contains(&"file1.txt"));
    kassert!(names.contains(&"file2.txt"));
    kassert!(names.contains(&"dir1"));
});

test_case!(test_tmpfs_sparse_file, {
    let fs = create_test_tmpfs();
    let root = fs.root_inode();

    let file = root
        .create("sparse.txt", FileMode::from_bits_truncate(0o644))
        .unwrap();

    // 写入第 0 页
    file.write_at(0, b"first page").unwrap();

    // 跳过中间，写入第 10 页
    let offset = 10 * 4096;
    file.write_at(offset, b"tenth page").unwrap();

    // 读取第 5 页（应该是全 0）
    let mut buf = vec![0xFFu8; 4096];
    let read = file.read_at(5 * 4096, &mut buf).unwrap();
    kassert!(read == 4096);
    kassert!(buf.iter().all(|&b| b == 0));

    // 读取第 10 页
    let mut buf = vec![0u8; 10];
    file.read_at(offset, &mut buf).unwrap();
    kassert!(&buf[..] == b"tenth page");
});

test_case!(test_tmpfs_truncate, {
    let fs = create_test_tmpfs();
    let inode = create_test_file_with_content(&fs, "truncate.txt", b"Hello, World!").unwrap();

    // 验证初始大小
    kassert!(inode.metadata().unwrap().size == 13);

    // 截断到 5 字节
    let result = inode.truncate(5);
    kassert!(result.is_ok());
    kassert!(inode.metadata().unwrap().size == 5);

    // 读取数据
    let mut buf = vec![0u8; 10];
    let read = inode.read_at(0, &mut buf).unwrap();
    kassert!(read == 5);
    kassert!(&buf[..5] == b"Hello");
});

test_case!(test_tmpfs_unlink, {
    let fs = create_test_tmpfs();
    let root = fs.root_inode();

    // 创建文件
    root.create("to_delete.txt", FileMode::from_bits_truncate(0o644))
        .unwrap();

    // 验证文件存在
    kassert!(root.lookup("to_delete.txt").is_ok());

    // 删除文件
    let result = root.unlink("to_delete.txt");
    kassert!(result.is_ok());

    // 验证文件不存在
    kassert!(root.lookup("to_delete.txt").is_err());
});

test_case!(test_tmpfs_rmdir, {
    let fs = create_test_tmpfs();
    let root = fs.root_inode();

    // 创建空目录
    root.mkdir("empty_dir", FileMode::from_bits_truncate(0o755))
        .unwrap();

    // 删除目录
    let result = root.rmdir("empty_dir");
    kassert!(result.is_ok());

    // 验证目录不存在
    kassert!(root.lookup("empty_dir").is_err());
});

test_case!(test_tmpfs_lookup_dot_dotdot, {
    let fs = create_test_tmpfs();
    let root = fs.root_inode();

    // 测试根目录的 "."
    let dot = root.lookup(".").unwrap();
    kassert!(dot.metadata().unwrap().inode_no == root.metadata().unwrap().inode_no);

    // 测试根目录的 ".." (应该指向自己)
    let dotdot = root.lookup("..").unwrap();
    kassert!(dotdot.metadata().unwrap().inode_no == root.metadata().unwrap().inode_no);

    // 创建子目录
    let subdir = root
        .mkdir("subdir", FileMode::from_bits_truncate(0o755))
        .unwrap();

    // 测试子目录的 ".."
    let parent = subdir.lookup("..").unwrap();
    kassert!(parent.metadata().unwrap().inode_no == root.metadata().unwrap().inode_no);
});

test_case!(test_tmpfs_already_exists, {
    let fs = create_test_tmpfs();
    let root = fs.root_inode();

    // 创建文件
    root.create("exists.txt", FileMode::from_bits_truncate(0o644))
        .unwrap();

    // 再次创建应该失败
    let result = root.create("exists.txt", FileMode::from_bits_truncate(0o644));
    kassert!(result.is_err());
    kassert!(matches!(result, Err(FsError::AlreadyExists)));
});

test_case!(test_tmpfs_capacity_limit, {
    // 创建无限制的 tmpfs
    let fs = create_test_tmpfs_unlimited();
    let root = fs.root_inode();

    let file = root
        .create("large.txt", FileMode::from_bits_truncate(0o644))
        .unwrap();

    // 写入多个页
    let data = vec![0xAAu8; 8192]; // 2 pages
    let result = file.write_at(0, &data);
    kassert!(result.is_ok());
});
