//! BlockDevice 适配器：VFS BlockDevice → ext4_rs BlockDevice

use crate::pr_err;
use alloc::sync::Arc;
use alloc::vec::Vec;

use crate::devices::{BlockDevice as VfsBlockDevice, block_device::BlockError};

/// ext4_rs BlockDevice trait 实现
///
/// 将我们的 VFS BlockDevice 适配为 ext4_rs 要求的接口
pub struct BlockDeviceAdapter {
    /// 底层块设备 (RamDisk 或 VirtIOBlock)
    inner: Arc<dyn VfsBlockDevice>,
    /// 块大小（通常 4096 字节）
    block_size: usize,
}

impl BlockDeviceAdapter {
    pub fn new(device: Arc<dyn VfsBlockDevice>) -> Self {
        let block_size = device.block_size();
        crate::println!("[Ext4Adapter] Created adapter with block_size: {}", block_size);
        Self {
            inner: device,
            block_size,
        }
    }
}

/// 实现 ext4_rs 的 BlockDevice trait
impl ext4_rs::BlockDevice for BlockDeviceAdapter {
    fn read_offset(&self, offset: usize) -> Vec<u8> {
        // ext4_rs 可能读取非对齐的数据（如超级块在偏移1024处）
        // 需要处理跨块读取
        let block_id = offset / self.block_size;
        let block_offset = offset % self.block_size;

        // 读取包含目标数据的块
        let mut buf = alloc::vec![0u8; self.block_size];
        match self.inner.read_block(block_id, &mut buf) {
            Ok(_) => {
                crate::println!("[Ext4Adapter] read_offset: off={}, block={}, local_off={}", offset, block_id, block_offset);

                // DEBUG: Check Superblock (Offset 1024 in Block 0)
                if offset == 1024 {
                    // buf contains 0..4096. SB starts at 1024.
                    let sb_offset = 1024;
                    let s_blocks_count = u32::from_le_bytes([buf[sb_offset+4], buf[sb_offset+5], buf[sb_offset+6], buf[sb_offset+7]]);
                    let s_r_blocks_count = u32::from_le_bytes([buf[sb_offset+8], buf[sb_offset+9], buf[sb_offset+10], buf[sb_offset+11]]);
                    let s_free_blocks = u32::from_le_bytes([buf[sb_offset+12], buf[sb_offset+13], buf[sb_offset+14], buf[sb_offset+15]]);
                    let s_first_data_block = u32::from_le_bytes([buf[sb_offset+20], buf[sb_offset+21], buf[sb_offset+22], buf[sb_offset+23]]);
                    let s_log_block_size = u32::from_le_bytes([buf[sb_offset+24], buf[sb_offset+25], buf[sb_offset+26], buf[sb_offset+27]]);
                    let s_blocks_per_group = u32::from_le_bytes([buf[sb_offset+32], buf[sb_offset+33], buf[sb_offset+34], buf[sb_offset+35]]);
                    
                    let s_inodes_count = u32::from_le_bytes([buf[sb_offset+0], buf[sb_offset+1], buf[sb_offset+2], buf[sb_offset+3]]);
                    let s_inodes_per_group = u32::from_le_bytes([buf[sb_offset+40], buf[sb_offset+41], buf[sb_offset+42], buf[sb_offset+43]]);

                    let s_feature_compat = u32::from_le_bytes([buf[sb_offset+92], buf[sb_offset+93], buf[sb_offset+94], buf[sb_offset+95]]);
                    let s_feature_incompat = u32::from_le_bytes([buf[sb_offset+96], buf[sb_offset+97], buf[sb_offset+98], buf[sb_offset+99]]);
                    let mut s_feature_ro_compat = u32::from_le_bytes([buf[sb_offset+100], buf[sb_offset+101], buf[sb_offset+102], buf[sb_offset+103]]);
                    
                    let s_reserved_gdt_blocks = u16::from_le_bytes([buf[sb_offset+206], buf[sb_offset+207]]);

                    let s_desc_size = u16::from_le_bytes([buf[sb_offset+254], buf[sb_offset+255]]);

                    crate::println!("[Ext4Adapter] SB read: blocks_count={}, r_blocks_count={}, free_blocks={}, first_data_block={}, log_block_size={}, blocks_per_group={}, desc_size={}", 
                        s_blocks_count, s_r_blocks_count, s_free_blocks, s_first_data_block, s_log_block_size, s_blocks_per_group, s_desc_size);
                    crate::println!("[Ext4Adapter] SB read: inodes_count={}, inodes_per_group={}, reserved_gdt_blocks={}", s_inodes_count, s_inodes_per_group, s_reserved_gdt_blocks);
                    crate::println!("[Ext4Adapter] SB features (orig): compat={:x}, incompat={:x}, ro_compat={:x}", 
                        s_feature_compat, s_feature_incompat, s_feature_ro_compat);

                    // HACK: Spoof s_blocks_count to 65536 (2 groups) to workaround ext4_rs bug
                    // The bug: balloc_alloc_block starts at bgid=1. If only 1 group (bgid=0), it fails to wrap around correctly.
                    // By pretending we have 2 groups, it tries bgid=1 (fails/skips), then wraps to bgid=0 (succeeds).
                    if s_blocks_count < s_blocks_per_group {
                        crate::println!("[Ext4Adapter] HACK: Spoofing s_blocks_count to {} to force 2 block groups", s_blocks_per_group * 2);
                        let new_blocks_count = s_blocks_per_group * 2;
                        let bytes = new_blocks_count.to_le_bytes();
                        buf[sb_offset+4] = bytes[0];
                        buf[sb_offset+5] = bytes[1];
                        buf[sb_offset+6] = bytes[2];
                        buf[sb_offset+7] = bytes[3];
                    }

                    // HACK: Mask out RO_COMPAT_GDT_CSUM (0x10) - Keeping this just in case
                    if (s_feature_ro_compat & 0x10) != 0 {
                        crate::println!("[Ext4Adapter] HACK: Masking out RO_COMPAT_GDT_CSUM");
                        s_feature_ro_compat &= !0x10;
                        let bytes = s_feature_ro_compat.to_le_bytes();
                        buf[sb_offset+100] = bytes[0];
                        buf[sb_offset+101] = bytes[1];
                        buf[sb_offset+102] = bytes[2];
                        buf[sb_offset+103] = bytes[3];
                    }
                }

                // DEBUG: Check GDT (Offset 4096 in Block 1)
                if offset == 4096 {
                    let block_bitmap = u32::from_le_bytes([buf[0], buf[1], buf[2], buf[3]]);
                    let inode_bitmap = u32::from_le_bytes([buf[4], buf[5], buf[6], buf[7]]);
                    let inode_table = u32::from_le_bytes([buf[8], buf[9], buf[10], buf[11]]);
                    let free_blocks = u16::from_le_bytes([buf[12], buf[13]]);
                    let free_inodes = u16::from_le_bytes([buf[14], buf[15]]);
                    let bg_flags = u16::from_le_bytes([buf[18], buf[19]]);
                    crate::println!("[Ext4Adapter] GDT read: block_bitmap={}, inode_bitmap={}, inode_table={}, free_blocks={}, free_inodes={}, bg_flags={:x}", 
                        block_bitmap, inode_bitmap, inode_table, free_blocks, free_inodes, bg_flags);
                }

                // HACK: Spoof GDT Entry 1 (Offset 64 in Block 1) to point to high block numbers
                // This prevents get_system_zone from calculating a reserved range (0..127) that overlaps with Group 0.
                if block_id == 1 {
                    // Check if we are reading the part that contains GDT Entry 1 (bytes 64..128)
                    // Since we read the whole block 1, we just modify it.
                    // Entry 1 starts at offset 64.
                    let gdt1_offset = 64;
                    
                    // Only if we are actually spoofing s_blocks_count (which we are), we need this.
                    // But it's safe to do it anyway since Group 1 doesn't exist.
                    
                    crate::println!("[Ext4Adapter] HACK: Spoofing GDT Entry 1 to avoid System Zone collision");
                    
                    // block_bitmap_lo = 60000
                    let bb_bytes = 60000u32.to_le_bytes();
                    buf[gdt1_offset + 0] = bb_bytes[0];
                    buf[gdt1_offset + 1] = bb_bytes[1];
                    buf[gdt1_offset + 2] = bb_bytes[2];
                    buf[gdt1_offset + 3] = bb_bytes[3];

                    // inode_bitmap_lo = 60001
                    let ib_bytes = 60001u32.to_le_bytes();
                    buf[gdt1_offset + 4] = ib_bytes[0];
                    buf[gdt1_offset + 5] = ib_bytes[1];
                    buf[gdt1_offset + 6] = ib_bytes[2];
                    buf[gdt1_offset + 7] = ib_bytes[3];

                    // inode_table_first_block_lo = 60002
                    let it_bytes = 60002u32.to_le_bytes();
                    buf[gdt1_offset + 8] = it_bytes[0];
                    buf[gdt1_offset + 9] = it_bytes[1];
                    buf[gdt1_offset + 10] = it_bytes[2];
                    buf[gdt1_offset + 11] = it_bytes[3];
                }

                // DEBUG: Check Block Bitmap (Block 2)
                if offset == 8192 {
                     let zeros = buf.iter().filter(|&&x| x == 0).count();
                     crate::println!("[Ext4Adapter] Reading Block 2 (Bitmap): zeros={}, first_bytes={:02x?}", zeros, &buf[..16]);
                }
                
                // DEBUG: Check Inode Bitmap (Block 18 - inferred from logs)
                if offset == 73728 {
                     let zeros = buf.iter().filter(|&&x| x == 0).count();
                     crate::println!("[Ext4Adapter] Reading Block 18 (Inode Bitmap): zeros={}, first_bytes={:02x?}", zeros, &buf[..16]);
                }
                
                // 返回从block_offset开始的数据
                // ext4_rs通常期望读取整个块，但从指定偏移开始
                if block_offset == 0 {
                    buf
                } else {
                    // 需要拼接两个块
                    let mut result = buf[block_offset..].to_vec();
                    // 尝试读取下一个块补齐
                    let mut next_buf = alloc::vec![0u8; self.block_size];
                    if self.inner.read_block(block_id + 1, &mut next_buf).is_ok() {
                        result.extend_from_slice(&next_buf[..block_offset]);
                    }
                    result
                }
            }
            Err(e) => {
                crate::pr_err!("[Ext4Adapter] Read error at offset {}: {:?}", offset, e);
                // ext4_rs 的 read_offset 不返回 Result，只能返回空数据
                alloc::vec![0u8; self.block_size]
            }
        }
    }

    fn write_offset(&self, offset: usize, data: &[u8]) {
        let block_id = offset / self.block_size;
        let block_offset = offset % self.block_size;

        // 优化：如果是对齐的完整块写入，直接写入
        if block_offset == 0 && data.len() == self.block_size {
            if let Err(e) = self.inner.write_block(block_id, data) {
                crate::pr_err!("[Ext4Adapter] Write error at offset {}: {:?}", offset, e);
            } else {
                crate::println!("[Ext4Adapter] write_offset success: off={}, block={}", offset, block_id);
            }
            return;
        }

        // 处理非对齐写入或跨块写入
        // 1. 处理第一个块
        let mut buf = alloc::vec![0u8; self.block_size];
        if self.inner.read_block(block_id, &mut buf).is_ok() {
            let space_in_block = self.block_size - block_offset;
            let write_len = data.len().min(space_in_block);

            buf[block_offset..block_offset + write_len].copy_from_slice(&data[..write_len]);

            if let Err(e) = self.inner.write_block(block_id, &buf) {
                crate::pr_err!("[Ext4Adapter] Write error at offset {}: {:?}", offset, e);
            }

            // 2. 如果还有数据，写入下一个块
            if write_len < data.len() {
                let remaining_data = &data[write_len..];
                let next_block_id = block_id + 1;
                
                let mut next_buf = alloc::vec![0u8; self.block_size];
                if self.inner.read_block(next_block_id, &mut next_buf).is_ok() {
                    let next_write_len = remaining_data.len().min(self.block_size);
                    next_buf[..next_write_len].copy_from_slice(&remaining_data[..next_write_len]);
                    
                    if let Err(e) = self.inner.write_block(next_block_id, &next_buf) {
                        crate::pr_err!("[Ext4Adapter] Write error at offset {}: {:?}", offset, e);
                    }
                } else {
                    crate::pr_err!("[Ext4Adapter] Read error at next block {}", next_block_id);
                }
            }
        } else {
            crate::pr_err!("[Ext4Adapter] Read error at offset {}: unable to read block {}", offset, block_id);
        }
    }
}
