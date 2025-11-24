use super::*;
use crate::vfs::file_system::FileSystem;
use crate::vfs::inode::InodeType;
use crate::{kassert, test_case};
use alloc::vec;

// 集成测试

test_case!(test_ext4_complete_workflow, {
    // 创建 Ext4 文件系统
    let fs = create_test_ext4();
    let root = fs.root_inode();

    // 1. 创建文件
    let inode = root
        .create("workflow.txt", FileMode::from_bits_truncate(0o644))
        .unwrap();

    // 2. 写入内容
    inode.write_at(0, b"Step 1").unwrap();
    inode.write_at(6, b" Step 2").unwrap();

    // 3. 读取验证
    let mut buf = vec![0u8; 13];
    let bytes_read = inode.read_at(0, &mut buf).unwrap();
    kassert!(bytes_read == 13);
    kassert!(&buf[..] == b"Step 1 Step 2");

    // 4. 查找文件
    let found_inode = root.lookup("workflow.txt").unwrap();
    let metadata = found_inode.metadata().unwrap();
    kassert!(metadata.size == 13);

    // 5. 删除文件
    root.unlink("workflow.txt").unwrap();

    // 6. 验证文件已删除
    let result = root.lookup("workflow.txt");
    kassert!(result.is_err());
});

test_case!(test_ext4_mixed_operations, {
    // 创建 Ext4 文件系统
    let fs = create_test_ext4();
    let root = fs.root_inode();

    // 创建混合的文件和目录
    root.create("file1.txt", FileMode::from_bits_truncate(0o644))
        .unwrap();
    let dir1 = root
        .mkdir("dir1", FileMode::from_bits_truncate(0o755))
        .unwrap();
    root.create("file2.txt", FileMode::from_bits_truncate(0o644))
        .unwrap();
    let dir2 = root
        .mkdir("dir2", FileMode::from_bits_truncate(0o755))
        .unwrap();

    // 在子目录中创建文件
    dir1.create("subfile1.txt", FileMode::from_bits_truncate(0o644))
        .unwrap();
    dir2.create("subfile2.txt", FileMode::from_bits_truncate(0o644))
        .unwrap();

    // 验证根目录内容
    let entries = root.readdir().unwrap();
    let names: alloc::vec::Vec<_> = entries.iter().map(|e| e.name.as_str()).collect();
    kassert!(names.contains(&"file1.txt"));
    kassert!(names.contains(&"file2.txt"));
    kassert!(names.contains(&"dir1"));
    kassert!(names.contains(&"dir2"));

    // 验证子目录内容
    let entries = dir1.readdir().unwrap();
    let names: alloc::vec::Vec<_> = entries.iter().map(|e| e.name.as_str()).collect();
    kassert!(names.contains(&"subfile1.txt"));
});

test_case!(test_ext4_nested_directory_structure, {
    // 创建多层嵌套目录结构
    let fs = create_test_ext4();
    let root = fs.root_inode();

    // 创建 /level1/level2/level3 结构
    let level1 = root
        .mkdir("level1", FileMode::from_bits_truncate(0o755))
        .unwrap();
    let level2 = level1
        .mkdir("level2", FileMode::from_bits_truncate(0o755))
        .unwrap();
    let level3 = level2
        .mkdir("level3", FileMode::from_bits_truncate(0o755))
        .unwrap();

    // 在最深层创建文件
    let deep_file = level3
        .create("deep.txt", FileMode::from_bits_truncate(0o644))
        .unwrap();
    deep_file.write_at(0, b"Deep content").unwrap();

    // 从根目录验证路径
    let found_level1 = root.lookup("level1").unwrap();
    let found_level2 = found_level1.lookup("level2").unwrap();
    let found_level3 = found_level2.lookup("level3").unwrap();
    let found_file = found_level3.lookup("deep.txt").unwrap();

    // 读取文件内容
    let mut buf = vec![0u8; 12];
    found_file.read_at(0, &mut buf).unwrap();
    kassert!(&buf[..] == b"Deep content");
});

test_case!(test_ext4_vfs_interface_completeness, {
    // 验证所有 VFS 接口方法都正常工作
    let fs = create_test_ext4();

    // FileSystem trait 方法
    kassert!(fs.fs_type() == "ext4");
    let root = fs.root_inode();
    kassert!(root.metadata().is_ok());
    kassert!(fs.statfs().is_ok());
    kassert!(fs.sync().is_ok());

    // Inode trait 方法 - 文件
    let file = root
        .create("test.txt", FileMode::from_bits_truncate(0o644))
        .unwrap();
    kassert!(file.metadata().is_ok());
    kassert!(file.write_at(0, b"test").is_ok());
    let mut buf = vec![0u8; 4];
    kassert!(file.read_at(0, &mut buf).is_ok());
    kassert!(file.truncate(2).is_ok());
    kassert!(file.sync().is_ok());

    // Inode trait 方法 - 目录
    let dir = root
        .mkdir("testdir", FileMode::from_bits_truncate(0o755))
        .unwrap();
    kassert!(dir.metadata().is_ok());
    kassert!(dir.readdir().is_ok());
    kassert!(dir.lookup("test.txt").is_err()); // 不在这个目录
    kassert!(
        dir.create("subfile.txt", FileMode::from_bits_truncate(0o644))
            .is_ok()
    );
    kassert!(dir.lookup("subfile.txt").is_ok());

    // 清理
    kassert!(root.unlink("test.txt").is_ok());
    kassert!(dir.unlink("subfile.txt").is_ok());
    kassert!(root.unlink("testdir").is_ok());
});

test_case!(test_ext4_filesystem_sync, {
    // 测试文件系统同步功能
    let fs = create_test_ext4();
    let root = fs.root_inode();

    // 创建多个文件并写入数据
    for i in 0..5 {
        let filename = alloc::format!("file{}.txt", i);
        let content = alloc::format!("Content {}", i);
        create_test_file_with_content(&fs, &filename, content.as_bytes()).unwrap();
    }

    // 同步文件系统
    let result = fs.sync();
    kassert!(result.is_ok());

    // 验证所有文件仍然存在且可读
    for i in 0..5 {
        let filename = alloc::format!("file{}.txt", i);
        let inode = root.lookup(&filename).unwrap();
        let expected = alloc::format!("Content {}", i);
        let mut buf = vec![0u8; expected.len()];
        inode.read_at(0, &mut buf).unwrap();
        kassert!(&buf[..] == expected.as_bytes());
    }
});
