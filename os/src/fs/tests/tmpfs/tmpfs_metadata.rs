//! Tmpfs 元数据和时间戳测试

use super::*;
use crate::{kassert, test_case};

test_case!(test_tmpfs_metadata_initial, {
    let fs = create_test_tmpfs();
    let root = fs.root_inode();
    let file = root
        .create("test.txt", FileMode::from_bits_truncate(0o644))
        .unwrap();

    let metadata = file.metadata().unwrap();
    kassert!(metadata.inode_type == InodeType::File);
    kassert!(metadata.size == 0);
    kassert!(metadata.nlinks == 1);
    kassert!(metadata.uid == 0);
    kassert!(metadata.gid == 0);
    kassert!(metadata.mode.bits() & 0o777 == 0o644);
});

test_case!(test_tmpfs_metadata_directory, {
    let fs = create_test_tmpfs();
    let root = fs.root_inode();
    let dir = root
        .mkdir("testdir", FileMode::from_bits_truncate(0o755))
        .unwrap();

    let metadata = dir.metadata().unwrap();
    kassert!(metadata.inode_type == InodeType::Directory);
    kassert!(metadata.nlinks >= 2); // . 和至少父目录
    kassert!(metadata.mode.bits() & 0o777 == 0o755);
});

test_case!(test_tmpfs_metadata_size_update, {
    let fs = create_test_tmpfs();
    let data = b"Hello, tmpfs!";
    let file = create_test_file_with_content(&fs, "test.txt", data).unwrap();

    let metadata = file.metadata().unwrap();
    kassert!(metadata.size == data.len());

    // 追加数据
    let more_data = b" More data.";
    kassert!(file.write_at(data.len(), more_data).is_ok());

    let metadata = file.metadata().unwrap();
    kassert!(metadata.size == data.len() + more_data.len());
});

test_case!(test_tmpfs_metadata_truncate_update, {
    let fs = create_test_tmpfs();
    let data = b"Original content that will be truncated";
    let file = create_test_file_with_content(&fs, "test.txt", data).unwrap();

    // 截断到更小
    kassert!(file.truncate(10).is_ok());
    let metadata = file.metadata().unwrap();
    kassert!(metadata.size == 10);

    // 截断到更大
    kassert!(file.truncate(100).is_ok());
    let metadata = file.metadata().unwrap();
    kassert!(metadata.size == 100);

    // 截断到0
    kassert!(file.truncate(0).is_ok());
    let metadata = file.metadata().unwrap();
    kassert!(metadata.size == 0);
});

test_case!(test_tmpfs_metadata_mode, {
    let fs = create_test_tmpfs();
    let root = fs.root_inode();

    // 测试不同的权限模式
    let file1 = root
        .create("file_644.txt", FileMode::from_bits_truncate(0o644))
        .unwrap();
    let meta1 = file1.metadata().unwrap();
    kassert!(meta1.mode.bits() & 0o777 == 0o644);

    let file2 = root
        .create("file_600.txt", FileMode::from_bits_truncate(0o600))
        .unwrap();
    let meta2 = file2.metadata().unwrap();
    kassert!(meta2.mode.bits() & 0o777 == 0o600);

    let dir1 = root
        .mkdir("dir_755", FileMode::from_bits_truncate(0o755))
        .unwrap();
    let meta3 = dir1.metadata().unwrap();
    kassert!(meta3.mode.bits() & 0o777 == 0o755);
});

test_case!(test_tmpfs_metadata_timestamps_file, {
    let fs = create_test_tmpfs();
    let root = fs.root_inode();
    let file = root
        .create("time_test.txt", FileMode::from_bits_truncate(0o644))
        .unwrap();

    let meta1 = file.metadata().unwrap();
    let ctime1 = meta1.ctime;
    let mtime1 = meta1.mtime;
    let _atime1 = meta1.atime;

    // 时间戳应该非零
    kassert!(ctime1.tv_sec > 0 || ctime1.tv_nsec > 0);

    // 写入数据（应更新 mtime）
    kassert!(file.write_at(0, b"test data").is_ok());

    let meta2 = file.metadata().unwrap();
    // mtime 应该更新（或至少不变小）
    kassert!(meta2.mtime.tv_sec >= mtime1.tv_sec);
});

test_case!(test_tmpfs_metadata_timestamps_dir, {
    let fs = create_test_tmpfs();
    let root = fs.root_inode();
    let dir = root
        .mkdir("timedir", FileMode::from_bits_truncate(0o755))
        .unwrap();

    let meta1 = dir.metadata().unwrap();
    kassert!(meta1.ctime.tv_sec > 0 || meta1.ctime.tv_nsec > 0);

    // 在目录中创建文件（应更新目录的 mtime）
    kassert!(
        dir.create("child.txt", FileMode::from_bits_truncate(0o644))
            .is_ok()
    );

    let meta2 = dir.metadata().unwrap();
    kassert!(meta2.mtime.tv_sec >= meta1.mtime.tv_sec);
});

test_case!(test_tmpfs_metadata_nlinks, {
    let fs = create_test_tmpfs();
    let root = fs.root_inode();

    // 文件的链接数
    let file = root
        .create("file.txt", FileMode::from_bits_truncate(0o644))
        .unwrap();
    let meta = file.metadata().unwrap();
    kassert!(meta.nlinks == 1);

    // 目录的链接数
    let dir = root
        .mkdir("dir1", FileMode::from_bits_truncate(0o755))
        .unwrap();
    let meta = dir.metadata().unwrap();
    kassert!(meta.nlinks >= 2); // . 和父目录

    // 在目录中创建子目录
    let _subdir = dir
        .mkdir("subdir", FileMode::from_bits_truncate(0o755))
        .unwrap();
    let meta = dir.metadata().unwrap();
    kassert!(meta.nlinks >= 3); // . 和父目录和子目录的 ..
});

test_case!(test_tmpfs_metadata_inode_number, {
    let fs = create_test_tmpfs();
    let root = fs.root_inode();

    // 每个文件应该有唯一的 inode 号
    let file1 = root
        .create("file1.txt", FileMode::from_bits_truncate(0o644))
        .unwrap();
    let file2 = root
        .create("file2.txt", FileMode::from_bits_truncate(0o644))
        .unwrap();
    let dir1 = root
        .mkdir("dir1", FileMode::from_bits_truncate(0o755))
        .unwrap();

    let meta1 = file1.metadata().unwrap();
    let meta2 = file2.metadata().unwrap();
    let meta3 = dir1.metadata().unwrap();

    kassert!(meta1.inode_no != meta2.inode_no);
    kassert!(meta1.inode_no != meta3.inode_no);
    kassert!(meta2.inode_no != meta3.inode_no);
});

test_case!(test_tmpfs_metadata_after_unlink, {
    let fs = create_test_tmpfs();
    let root = fs.root_inode();
    let file = root
        .create("temp.txt", FileMode::from_bits_truncate(0o644))
        .unwrap();

    // 删除前元数据应该有效
    let meta1 = file.metadata().unwrap();
    kassert!(meta1.nlinks == 1);

    // 删除文件
    kassert!(root.unlink("temp.txt").is_ok());

    // 如果还持有 inode 引用，元数据仍应可读（但 nlinks 可能变为0）
    let meta2 = file.metadata();
    kassert!(meta2.is_ok());
});
