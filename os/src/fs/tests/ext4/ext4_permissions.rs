use super::*;
use crate::vfs::inode::InodeType;
use crate::{kassert, test_case};

// P1 重要功能测试 - chown/chmod

test_case!(test_ext4_chown_basic, {
    // 创建文件
    let fs = create_test_ext4();
    let content = b"Test content";
    let inode = create_test_file_with_content(&fs, "test.txt", content).unwrap();

    // 获取初始元数据
    let metadata = inode.metadata().unwrap();
    let original_uid = metadata.uid;
    let original_gid = metadata.gid;

    // 修改所有者
    let new_uid = 1000;
    let new_gid = 1000;
    let result = inode.chown(new_uid, new_gid);
    kassert!(result.is_ok());

    // 验证修改成功
    let metadata = inode.metadata().unwrap();
    kassert!(metadata.uid == new_uid);
    kassert!(metadata.gid == new_gid);
    kassert!(metadata.uid != original_uid || original_uid == new_uid);
    kassert!(metadata.gid != original_gid || original_gid == new_gid);
});

test_case!(test_ext4_chown_uid_only, {
    // 创建文件
    let fs = create_test_ext4();
    let inode = create_test_file(&fs, "test.txt").unwrap();

    // 获取初始 gid
    let original_gid = inode.metadata().unwrap().gid;

    // 只修改 uid，gid 传 u32::MAX 表示不改变
    let new_uid = 2000;
    let result = inode.chown(new_uid, u32::MAX);
    kassert!(result.is_ok());

    // 验证：uid 改变，gid 不变
    let metadata = inode.metadata().unwrap();
    kassert!(metadata.uid == new_uid);
    kassert!(metadata.gid == original_gid);
});

test_case!(test_ext4_chown_gid_only, {
    // 创建文件
    let fs = create_test_ext4();
    let inode = create_test_file(&fs, "test.txt").unwrap();

    // 获取初始 uid
    let original_uid = inode.metadata().unwrap().uid;

    // 只修改 gid，uid 传 u32::MAX 表示不改变
    let new_gid = 3000;
    let result = inode.chown(u32::MAX, new_gid);
    kassert!(result.is_ok());

    // 验证：gid 改变，uid 不变
    let metadata = inode.metadata().unwrap();
    kassert!(metadata.uid == original_uid);
    kassert!(metadata.gid == new_gid);
});

test_case!(test_ext4_chown_directory, {
    // 创建目录
    let fs = create_test_ext4();
    let dir = create_test_dir(&fs, "testdir").unwrap();

    // 修改目录所有者
    let new_uid = 1001;
    let new_gid = 1001;
    let result = dir.chown(new_uid, new_gid);
    kassert!(result.is_ok());

    // 验证修改成功
    let metadata = dir.metadata().unwrap();
    kassert!(metadata.inode_type == InodeType::Directory);
    kassert!(metadata.uid == new_uid);
    kassert!(metadata.gid == new_gid);
});

test_case!(test_ext4_chmod_basic, {
    // 创建文件
    let fs = create_test_ext4();
    let inode = create_test_file(&fs, "test.txt").unwrap();

    // 修改权限为 0o755 (rwxr-xr-x)
    let new_mode = FileMode::from_bits_truncate(0o755);
    let result = inode.chmod(new_mode);
    kassert!(result.is_ok());

    // 验证修改成功
    let metadata = inode.metadata().unwrap();
    kassert!(metadata.mode.contains(FileMode::S_IRUSR)); // owner read
    kassert!(metadata.mode.contains(FileMode::S_IWUSR)); // owner write
    kassert!(metadata.mode.contains(FileMode::S_IXUSR)); // owner execute
    kassert!(metadata.mode.contains(FileMode::S_IRGRP)); // group read
    kassert!(metadata.mode.contains(FileMode::S_IXGRP)); // group execute
    kassert!(metadata.mode.contains(FileMode::S_IROTH)); // other read
    kassert!(metadata.mode.contains(FileMode::S_IXOTH)); // other execute
    kassert!(!metadata.mode.contains(FileMode::S_IWGRP)); // no group write
    kassert!(!metadata.mode.contains(FileMode::S_IWOTH)); // no other write
});

test_case!(test_ext4_chmod_readonly, {
    // 创建文件
    let fs = create_test_ext4();
    let inode = create_test_file(&fs, "test.txt").unwrap();

    // 修改权限为 0o444 (r--r--r--)
    let new_mode = FileMode::from_bits_truncate(0o444);
    let result = inode.chmod(new_mode);
    kassert!(result.is_ok());

    // 验证修改成功
    let metadata = inode.metadata().unwrap();
    kassert!(metadata.mode.contains(FileMode::S_IRUSR));
    kassert!(metadata.mode.contains(FileMode::S_IRGRP));
    kassert!(metadata.mode.contains(FileMode::S_IROTH));
    kassert!(!metadata.mode.contains(FileMode::S_IWUSR));
    kassert!(!metadata.mode.contains(FileMode::S_IWGRP));
    kassert!(!metadata.mode.contains(FileMode::S_IWOTH));
    kassert!(!metadata.mode.contains(FileMode::S_IXUSR));
    kassert!(!metadata.mode.contains(FileMode::S_IXGRP));
    kassert!(!metadata.mode.contains(FileMode::S_IXOTH));
});

test_case!(test_ext4_chmod_special_bits, {
    // 创建文件
    let fs = create_test_ext4();
    let inode = create_test_file(&fs, "test.txt").unwrap();

    // 设置特殊权限位：setuid(4), setgid(2), sticky(1)
    let new_mode = FileMode::from_bits_truncate(0o6755); // setuid + setgid + rwxr-xr-x
    let result = inode.chmod(new_mode);
    kassert!(result.is_ok());

    // 验证特殊位设置成功
    let metadata = inode.metadata().unwrap();
    kassert!(metadata.mode.contains(FileMode::S_ISUID)); // setuid
    kassert!(metadata.mode.contains(FileMode::S_ISGID)); // setgid
    kassert!(!metadata.mode.contains(FileMode::S_ISVTX)); // no sticky bit
});

test_case!(test_ext4_chmod_directory, {
    // 创建目录
    let fs = create_test_ext4();
    let dir = create_test_dir(&fs, "testdir").unwrap();

    // 修改目录权限为 0o700 (rwx------)
    let new_mode = FileMode::from_bits_truncate(0o700);
    let result = dir.chmod(new_mode);
    kassert!(result.is_ok());

    // 验证修改成功
    let metadata = dir.metadata().unwrap();
    kassert!(metadata.inode_type == InodeType::Directory);
    kassert!(metadata.mode.contains(FileMode::S_IRUSR));
    kassert!(metadata.mode.contains(FileMode::S_IWUSR));
    kassert!(metadata.mode.contains(FileMode::S_IXUSR));
    kassert!(!metadata.mode.contains(FileMode::S_IRGRP));
    kassert!(!metadata.mode.contains(FileMode::S_IROTH));
});

test_case!(test_ext4_chmod_preserves_file_type, {
    // 创建文件和目录
    let fs = create_test_ext4();
    let file = create_test_file(&fs, "file.txt").unwrap();
    let dir = create_test_dir(&fs, "dir").unwrap();

    // 修改权限
    file.chmod(FileMode::from_bits_truncate(0o644)).unwrap();
    dir.chmod(FileMode::from_bits_truncate(0o644)).unwrap();

    // 验证文件类型不变
    let file_meta = file.metadata().unwrap();
    let dir_meta = dir.metadata().unwrap();
    kassert!(file_meta.inode_type == InodeType::File);
    kassert!(dir_meta.inode_type == InodeType::Directory);
});

test_case!(test_ext4_chown_chmod_combined, {
    // 创建文件
    let fs = create_test_ext4();
    let inode = create_test_file(&fs, "test.txt").unwrap();

    // 先 chown
    inode.chown(1234, 5678).unwrap();

    // 再 chmod
    let new_mode = FileMode::from_bits_truncate(0o600);
    inode.chmod(new_mode).unwrap();

    // 验证两者都生效
    let metadata = inode.metadata().unwrap();
    kassert!(metadata.uid == 1234);
    kassert!(metadata.gid == 5678);
    kassert!(metadata.mode.contains(FileMode::S_IRUSR));
    kassert!(metadata.mode.contains(FileMode::S_IWUSR));
    kassert!(!metadata.mode.contains(FileMode::S_IXUSR));
    kassert!(!metadata.mode.contains(FileMode::S_IRGRP));
    kassert!(!metadata.mode.contains(FileMode::S_IROTH));
});
