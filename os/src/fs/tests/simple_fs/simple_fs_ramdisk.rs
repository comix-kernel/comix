use super::*;
use crate::device::block::BlockDriver;
use crate::fs::simple_fs::SimpleFs;
use crate::{kassert, test_case};
use alloc::vec;

// P0 核心功能测试

test_case!(test_simplefs_from_empty_ramdisk, {
    // 创建一个空的 RamDisk 镜像
    let ramdisk = create_test_ramdisk_with_files();

    // 从 RamDisk 加载 SimpleFS
    let result = SimpleFs::from_ramdisk(ramdisk);
    kassert!(result.is_ok());
});

test_case!(test_simplefs_ramdisk_magic_verification, {
    // 创建一个无效魔数的 RamDisk
    let mut data = alloc::vec![0u8; 512];
    data[0..8].copy_from_slice(b"INVALID\0");
    let ramdisk = RamDisk::from_bytes(data, 512, 0);

    // 尝试加载应该失败
    let result = SimpleFs::from_ramdisk(ramdisk);
    kassert!(result.is_err());
    kassert!(matches!(result, Err(FsError::IoError)));
});

test_case!(test_simplefs_ramdisk_file_count, {
    // 创建包含正确头部的 RamDisk
    let mut data = alloc::vec![0u8; 512];
    data[0..8].copy_from_slice(b"RAMDISK\0");
    // File count: 0
    data[8..12].copy_from_slice(&0u32.to_le_bytes());

    let ramdisk = RamDisk::from_bytes(data, 512, 0);

    // 加载 SimpleFS
    let result = SimpleFs::from_ramdisk(ramdisk);
    kassert!(result.is_ok());
});

// P1 重要功能测试

test_case!(test_simplefs_ramdisk_block_size, {
    // 创建 RamDisk
    let ramdisk = create_test_ramdisk_with_files();

    // 验证块大小
    kassert!(ramdisk.block_size() == 512);
    kassert!(ramdisk.total_blocks() == 1);
});

// P2 边界和错误处理测试

test_case!(test_simplefs_ramdisk_too_small, {
    // 创建太小的 RamDisk（小于一个块）
    let data = alloc::vec![0u8; 256];
    let ramdisk = RamDisk::from_bytes(data, 512, 0);

    // 尝试读取第一个块应该失败
    let mut buf = vec![0u8; 512];
    let result = ramdisk.read_block(0, &mut buf);
    kassert!(!result);
});
