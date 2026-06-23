use super::*;
use crate::vfs::file_system::FileSystem;
use crate::vfs::inode::InodeType;
use crate::{kassert, test_case};

// P1 重要功能测试

test_case!(test_ext4_file_metadata, {
    // 创建文件
    let fs = create_test_ext4();
    let content = b"Test content";
    let inode = create_test_file_with_content(&fs, "test.txt", content).unwrap();

    // 获取元数据
    let metadata = inode.metadata().unwrap();
    kassert!(metadata.inode_type == InodeType::File);
    kassert!(metadata.size == content.len());
    kassert!(metadata.mode.can_read());
    kassert!(metadata.mode.can_write());
});

test_case!(test_ext4_directory_metadata, {
    // 创建目录
    let fs = create_test_ext4();
    let dir = create_test_dir(&fs, "testdir").unwrap();

    // 获取元数据
    let metadata = dir.metadata().unwrap();
    kassert!(metadata.inode_type == InodeType::Directory);
    kassert!(metadata.mode.can_read());
    kassert!(metadata.mode.can_write());
    kassert!(metadata.mode.can_execute());
});

test_case!(test_ext4_statfs, {
    // 创建 Ext4 文件系统
    let fs = create_test_ext4();

    // 获取文件系统统计信息
    let statfs = fs.statfs().unwrap();

    // 验证基本信息
    kassert!(statfs.block_size > 0);
    kassert!(statfs.total_blocks > 0);
    kassert!(statfs.total_inodes > 0);
    kassert!(statfs.max_filename_len == 255); // Ext4 标准
});

test_case!(test_ext4_inode_number, {
    // 创建文件系统
    let fs = create_test_ext4();
    let root = fs.root_inode();

    // 获取根 inode 元数据
    let root_metadata = root.metadata().unwrap();
    kassert!(root_metadata.inode_no == 2); // Ext4 根目录 inode 号为 2

    // 创建文件并验证 inode 号不同
    let inode = root
        .create("test.txt", FileMode::from_bits_truncate(0o644))
        .unwrap();
    let file_metadata = inode.metadata().unwrap();
    kassert!(file_metadata.inode_no != root_metadata.inode_no);
});

test_case!(test_ext4_create_preserves_file_mode, {
    let fs = create_test_ext4();
    let root = fs.root_inode();

    let inode = root
        .create("private.txt", FileMode::from_bits_truncate(0o600))
        .unwrap();
    let metadata = inode.metadata().unwrap();

    kassert!(metadata.inode_type == InodeType::File);
    kassert!((metadata.mode.bits() & 0o777) == 0o600);
});

test_case!(test_ext4_mkdir_preserves_mode, {
    let fs = create_test_ext4();
    let root = fs.root_inode();

    let inode = root
        .mkdir("private-dir", FileMode::from_bits_truncate(0o700))
        .unwrap();
    let metadata = inode.metadata().unwrap();

    kassert!(metadata.inode_type == InodeType::Directory);
    kassert!((metadata.mode.bits() & 0o777) == 0o700);
});

test_case!(test_ext4_mknod_device_metadata, {
    use crate::vfs::dev::makedev;

    let fs = create_test_ext4();
    let root = fs.root_inode();
    let chr_dev = makedev(1, 3);
    let blk_dev = makedev(254, 16);

    let chr = root
        .mknod(
            "null",
            FileMode::S_IFCHR | FileMode::from_bits_truncate(0o666),
            chr_dev,
        )
        .unwrap();
    let blk = root
        .mknod(
            "vdb",
            FileMode::S_IFBLK | FileMode::from_bits_truncate(0o660),
            blk_dev,
        )
        .unwrap();

    let chr_meta = chr.metadata().unwrap();
    let blk_meta = blk.metadata().unwrap();
    kassert!(chr_meta.inode_type == InodeType::CharDevice);
    kassert!(chr_meta.rdev == chr_dev);
    kassert!((chr_meta.mode.bits() & 0o777) == 0o666);
    kassert!(blk_meta.inode_type == InodeType::BlockDevice);
    kassert!(blk_meta.rdev == blk_dev);
    kassert!((blk_meta.mode.bits() & 0o777) == 0o660);
});

test_case!(test_ext4_mknod_special_file_types, {
    let fs = create_test_ext4();
    let root = fs.root_inode();

    let fifo = root
        .mknod(
            "fifo",
            FileMode::S_IFIFO | FileMode::from_bits_truncate(0o644),
            0,
        )
        .unwrap();
    let sock = root
        .mknod(
            "sock",
            FileMode::S_IFSOCK | FileMode::from_bits_truncate(0o644),
            0,
        )
        .unwrap();
    let reg = root
        .mknod(
            "regular",
            FileMode::S_IFREG | FileMode::from_bits_truncate(0o600),
            0,
        )
        .unwrap();

    kassert!(fifo.metadata().unwrap().inode_type == InodeType::Fifo);
    kassert!(sock.metadata().unwrap().inode_type == InodeType::Socket);
    let reg_meta = reg.metadata().unwrap();
    kassert!(reg_meta.inode_type == InodeType::File);
    kassert!((reg_meta.mode.bits() & 0o777) == 0o600);
});

test_case!(test_ext4_mknod_duplicate, {
    let fs = create_test_ext4();
    let root = fs.root_inode();

    root.mknod(
        "fifo",
        FileMode::S_IFIFO | FileMode::from_bits_truncate(0o644),
        0,
    )
    .unwrap();

    let result = root.mknod(
        "fifo",
        FileMode::S_IFIFO | FileMode::from_bits_truncate(0o644),
        0,
    );
    kassert!(matches!(result, Err(FsError::AlreadyExists)));
});
