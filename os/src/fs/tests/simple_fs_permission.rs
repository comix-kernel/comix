use super::*;
use crate::vfs::file_system::FileSystem;
use crate::{kassert, test_case};

// P1 重要功能测试

test_case!(test_simplefs_file_permissions_read, {
    // 创建只读文件 (0o444)
    let fs = create_test_simplefs();
    let root = fs.root_inode();
    let inode = root
        .create("readonly.txt", FileMode::from_bits_truncate(0o444))
        .unwrap();

    // 验证权限
    let metadata = inode.metadata().unwrap();
    kassert!(metadata.mode.can_read());
    kassert!(!metadata.mode.can_write());
});

test_case!(test_simplefs_file_permissions_write, {
    // 创建只写文件 (0o222)
    let fs = create_test_simplefs();
    let root = fs.root_inode();
    let inode = root
        .create("writeonly.txt", FileMode::from_bits_truncate(0o222))
        .unwrap();

    // 验证权限
    let metadata = inode.metadata().unwrap();
    kassert!(!metadata.mode.can_read());
    kassert!(metadata.mode.can_write());
});

test_case!(test_simplefs_file_permissions_execute, {
    // 创建可执行文件 (0o755)
    let fs = create_test_simplefs();
    let root = fs.root_inode();
    let inode = root
        .create("executable", FileMode::from_bits_truncate(0o755))
        .unwrap();

    // 验证权限
    let metadata = inode.metadata().unwrap();
    kassert!(metadata.mode.can_read());
    kassert!(metadata.mode.can_write());
    kassert!(metadata.mode.can_execute());
});

test_case!(test_simplefs_directory_permissions, {
    // 创建目录 (0o755)
    let fs = create_test_simplefs();
    let root = fs.root_inode();
    let dir = root
        .mkdir("testdir", FileMode::from_bits_truncate(0o755))
        .unwrap();

    // 验证权限
    let metadata = dir.metadata().unwrap();
    kassert!(metadata.mode.can_read());
    kassert!(metadata.mode.can_write());
    kassert!(metadata.mode.can_execute());
});

test_case!(test_simplefs_permissions_from_bits, {
    // 测试 FileMode::from_bits_truncate
    let mode_644 = FileMode::from_bits_truncate(0o644);
    kassert!(mode_644.can_read());
    kassert!(mode_644.can_write());
    kassert!(!mode_644.can_execute());

    let mode_755 = FileMode::from_bits_truncate(0o755);
    kassert!(mode_755.can_read());
    kassert!(mode_755.can_write());
    kassert!(mode_755.can_execute());

    let mode_444 = FileMode::from_bits_truncate(0o444);
    kassert!(mode_444.can_read());
    kassert!(!mode_444.can_write());
    kassert!(!mode_444.can_execute());
});

// P2 边界和错误处理测试

test_case!(test_simplefs_no_permissions, {
    // 创建没有任何权限的文件 (0o000)
    let fs = create_test_simplefs();
    let root = fs.root_inode();
    let inode = root
        .create("noperm.txt", FileMode::from_bits_truncate(0o000))
        .unwrap();

    // 验证权限
    let metadata = inode.metadata().unwrap();
    kassert!(!metadata.mode.can_read());
    kassert!(!metadata.mode.can_write());
    kassert!(!metadata.mode.can_execute());
});
