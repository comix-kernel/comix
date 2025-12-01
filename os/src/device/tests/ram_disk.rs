use super::super::*;
use crate::device::block::BlockDriver;
use crate::device::ram_disk::RamDisk;
use crate::{kassert, test_case};

// P0 核心功能测试

test_case!(test_ramdisk_create_new, {
    // 创建一个 1KB 的 RamDisk (2 块 * 512 bytes)
    let ramdisk = RamDisk::new(1024, 512, 0);

    kassert!(ramdisk.block_size() == 512);
    kassert!(ramdisk.total_blocks() == 2);
    kassert!(ramdisk.device_id() == 0);
});

test_case!(test_ramdisk_from_bytes, {
    // 创建一个包含测试数据的 RamDisk
    let data = alloc::vec![0x42u8; 1024];
    let ramdisk = RamDisk::from_bytes(data, 512, 1);

    kassert!(ramdisk.block_size() == 512);
    kassert!(ramdisk.total_blocks() == 2);
    kassert!(ramdisk.device_id() == 1);

    // 验证数据被正确存储
    let raw_data = ramdisk.raw_data();
    kassert!(raw_data.len() == 1024);
    kassert!(raw_data[0] == 0x42);
    kassert!(raw_data[1023] == 0x42);
});

test_case!(test_ramdisk_read_block, {
    // 创建一个包含测试数据的 RamDisk
    let mut data = alloc::vec![0u8; 1024];
    // 第一个块填充 0xAA
    for i in 0..512 {
        data[i] = 0xAA;
    }
    // 第二个块填充 0xBB
    for i in 512..1024 {
        data[i] = 0xBB;
    }

    let ramdisk = RamDisk::from_bytes(data, 512, 0);

    // 读取第一个块
    let mut buf = [0u8; 512];
    let result = ramdisk.read_block(0, &mut buf);
    kassert!(result);
    kassert!(buf[0] == 0xAA);
    kassert!(buf[511] == 0xAA);

    // 读取第二个块
    let result = ramdisk.read_block(1, &mut buf);
    kassert!(result);
    kassert!(buf[0] == 0xBB);
    kassert!(buf[511] == 0xBB);
});

test_case!(test_ramdisk_write_block, {
    // 创建一个空的 RamDisk
    let ramdisk = RamDisk::new(1024, 512, 0);

    // 写入第一个块
    let write_buf = [0xCC; 512];
    let result = ramdisk.write_block(0, &write_buf);
    kassert!(result);

    // 读回验证
    let mut read_buf = [0u8; 512];
    kassert!(ramdisk.read_block(0, &mut read_buf));
    kassert!(read_buf[0] == 0xCC);
    kassert!(read_buf[511] == 0xCC);
});

test_case!(test_ramdisk_flush, {
    let ramdisk = RamDisk::new(512, 512, 0);

    // flush 应该始终成功 (内存设备无需 flush)
    let result = ramdisk.flush();
    kassert!(result);
});

// P2 边界和错误处理测试

test_case!(test_ramdisk_read_invalid_block, {
    let ramdisk = RamDisk::new(512, 512, 0);

    // 尝试读取超出范围的块
    let mut buf = [0u8; 512];
    let result = ramdisk.read_block(1, &mut buf);
    kassert!(!result);
});

test_case!(test_ramdisk_write_invalid_block, {
    let ramdisk = RamDisk::new(512, 512, 0);

    // 尝试写入超出范围的块
    let buf = [0u8; 512];
    let result = ramdisk.write_block(1, &buf);
    kassert!(!result);
});

test_case!(test_ramdisk_read_wrong_buffer_size, {
    let ramdisk = RamDisk::new(512, 512, 0);

    // 使用错误的缓冲区大小
    let mut buf = [0u8; 256];
    let result = ramdisk.read_block(0, &mut buf);
    kassert!(!result);
});

test_case!(test_ramdisk_write_wrong_buffer_size, {
    let ramdisk = RamDisk::new(512, 512, 0);

    // 使用错误的缓冲区大小
    let buf = [0u8; 256];
    let result = ramdisk.write_block(0, &buf);
    kassert!(!result);
});

// P1 重要功能测试

test_case!(test_ramdisk_multiple_read_write, {
    let ramdisk = RamDisk::new(2048, 512, 0);

    // 写入多个块
    for block_id in 0..4 {
        let write_buf = [block_id as u8; 512];
        let result = ramdisk.write_block(block_id, &write_buf);
        kassert!(result);
    }

    // 读回验证
    for block_id in 0..4 {
        let mut read_buf = [0u8; 512];
        kassert!(ramdisk.read_block(block_id, &mut read_buf));
        kassert!(read_buf[0] == block_id as u8);
        kassert!(read_buf[511] == block_id as u8);
    }
});

test_case!(test_ramdisk_overwrite_block, {
    let ramdisk = RamDisk::new(512, 512, 0);

    // 第一次写入
    let write_buf1 = [0xAA; 512];
    kassert!(ramdisk.write_block(0, &write_buf1));

    // 覆盖写入
    let write_buf2 = [0xBB; 512];
    kassert!(ramdisk.write_block(0, &write_buf2));

    // 读回验证
    let mut read_buf = [0u8; 512];
    kassert!(ramdisk.read_block(0, &mut read_buf));
    kassert!(read_buf[0] == 0xBB);
    kassert!(read_buf[511] == 0xBB);
});
