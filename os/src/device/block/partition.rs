//! Partition block-device wrapper and simple MBR discovery.

use super::BlockDriver;
use crate::device::{DeviceType, Driver};
use alloc::string::String;
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::cmp::min;

const MBR_SIGNATURE_OFFSET: usize = 510;
const MBR_PARTITION_TABLE_OFFSET: usize = 446;
const MBR_PARTITION_ENTRY_SIZE: usize = 16;
const MBR_PARTITION_COUNT: usize = 4;
const GPT_HEADER_LBA: usize = 1;
const GPT_SIGNATURE: &[u8; 8] = b"EFI PART";
const GPT_ENTRY_PREFIX_LEN: usize = 56;
const GPT_ENTRY_TYPE_GUID_LEN: usize = 16;
const MAX_PARTITION_NUMBER: u32 = 15;

/// Parsed on-disk partition range.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PartitionEntry {
    /// 1-based partition number in the disk partition table.
    pub number: u32,
    /// Partition type byte from the MBR entry.
    pub type_code: u8,
    /// Start sector/LBA in 512-byte sectors.
    pub start_lba: usize,
    /// Length in 512-byte sectors.
    pub sector_count: usize,
}

impl PartitionEntry {
    pub fn is_empty(self) -> bool {
        self.type_code == 0 || self.sector_count == 0
    }
}

/// A logical block device backed by a contiguous block range of another device.
pub struct PartitionBlockDevice {
    inner: Arc<dyn BlockDriver>,
    name: String,
    start_block: usize,
    block_count: usize,
}

impl PartitionBlockDevice {
    pub fn new(
        inner: Arc<dyn BlockDriver>,
        name: String,
        start_block: usize,
        block_count: usize,
    ) -> Option<Arc<Self>> {
        let end_block = start_block.checked_add(block_count)?;
        if block_count == 0 || end_block > inner.total_blocks() {
            return None;
        }

        Some(Arc::new(Self {
            inner,
            name,
            start_block,
            block_count,
        }))
    }
}

impl Driver for PartitionBlockDevice {
    fn try_handle_interrupt(&self, _irq: Option<usize>) -> bool {
        false
    }

    fn device_type(&self) -> DeviceType {
        DeviceType::Block
    }

    fn get_id(&self) -> String {
        self.name.clone()
    }

    fn as_block(&self) -> Option<&dyn BlockDriver> {
        Some(self)
    }

    fn as_block_arc(self: Arc<Self>) -> Option<Arc<dyn BlockDriver>> {
        Some(self)
    }
}

impl BlockDriver for PartitionBlockDevice {
    fn read_block(&self, block_id: usize, buf: &mut [u8]) -> bool {
        if block_id >= self.block_count {
            return false;
        }

        let Some(inner_block) = self.start_block.checked_add(block_id) else {
            return false;
        };
        self.inner.read_block(inner_block, buf)
    }

    fn write_block(&self, block_id: usize, buf: &[u8]) -> bool {
        if block_id >= self.block_count {
            return false;
        }

        let Some(inner_block) = self.start_block.checked_add(block_id) else {
            return false;
        };
        self.inner.write_block(inner_block, buf)
    }

    fn flush(&self) -> bool {
        self.inner.flush()
    }

    fn block_size(&self) -> usize {
        self.inner.block_size()
    }

    fn total_blocks(&self) -> usize {
        self.block_count
    }
}

/// Discover primary MBR partitions for a 512-byte-sector block device.
pub fn read_mbr_partitions(device: &Arc<dyn BlockDriver>) -> [Option<PartitionEntry>; 4] {
    let mut partitions = [None; 4];
    if device.block_size() != 512 || device.total_blocks() == 0 {
        return partitions;
    }

    let mut sector = [0u8; 512];
    if !device.read_block(0, &mut sector) {
        return partitions;
    }

    if sector[MBR_SIGNATURE_OFFSET] != 0x55 || sector[MBR_SIGNATURE_OFFSET + 1] != 0xAA {
        return partitions;
    }

    for (idx, slot) in partitions.iter_mut().enumerate().take(MBR_PARTITION_COUNT) {
        let offset = MBR_PARTITION_TABLE_OFFSET + idx * MBR_PARTITION_ENTRY_SIZE;
        let type_code = sector[offset + 4];
        let start_lba = u32::from_le_bytes([
            sector[offset + 8],
            sector[offset + 9],
            sector[offset + 10],
            sector[offset + 11],
        ]) as usize;
        let sector_count = u32::from_le_bytes([
            sector[offset + 12],
            sector[offset + 13],
            sector[offset + 14],
            sector[offset + 15],
        ]) as usize;

        let entry = PartitionEntry {
            number: (idx + 1) as u32,
            type_code,
            start_lba,
            sector_count,
        };
        if !entry.is_empty() {
            *slot = Some(entry);
        }
    }

    partitions
}

/// Discover disk partitions using GPT when present, otherwise primary MBR.
pub fn discover_partitions(device: &Arc<dyn BlockDriver>) -> Vec<PartitionEntry> {
    let mbr_partitions = read_mbr_partitions(device);
    let has_protective_mbr = mbr_partitions
        .iter()
        .flatten()
        .any(|entry| entry.type_code == 0xEE);

    if has_protective_mbr {
        return read_gpt_partitions(device);
    }

    mbr_partitions
        .iter()
        .flatten()
        .copied()
        .filter(|entry| entry.number <= MAX_PARTITION_NUMBER)
        .collect()
}

fn read_gpt_partitions(device: &Arc<dyn BlockDriver>) -> Vec<PartitionEntry> {
    let mut partitions = Vec::new();
    if device.block_size() != 512 || device.total_blocks() <= GPT_HEADER_LBA {
        return partitions;
    }

    let mut header = [0u8; 512];
    if !device.read_block(GPT_HEADER_LBA, &mut header) {
        return partitions;
    }

    if &header[..GPT_SIGNATURE.len()] != GPT_SIGNATURE {
        return partitions;
    }

    let entries_lba = le_u64(&header[72..80]) as usize;
    let entry_count = le_u32(&header[80..84]) as usize;
    let entry_size = le_u32(&header[84..88]) as usize;
    if entries_lba >= device.total_blocks() || entry_size < 128 {
        return partitions;
    }

    let max_entries = min(entry_count, MAX_PARTITION_NUMBER as usize);
    for index in 0..max_entries {
        let mut entry = [0u8; GPT_ENTRY_PREFIX_LEN];
        if !read_gpt_entry_prefix(device, entries_lba, entry_size, index, &mut entry) {
            break;
        }

        if entry[..GPT_ENTRY_TYPE_GUID_LEN]
            .iter()
            .all(|byte| *byte == 0)
        {
            continue;
        }

        let first_lba = le_u64(&entry[32..40]);
        let last_lba = le_u64(&entry[40..48]);
        if first_lba == 0 || last_lba < first_lba {
            continue;
        }

        let Some(sector_count) = last_lba
            .checked_sub(first_lba)
            .and_then(|count| count.checked_add(1))
            .and_then(|count| usize::try_from(count).ok())
        else {
            continue;
        };
        let Ok(start_lba) = usize::try_from(first_lba) else {
            continue;
        };

        partitions.push(PartitionEntry {
            number: (index + 1) as u32,
            type_code: 0,
            start_lba,
            sector_count,
        });
    }

    partitions
}

fn read_gpt_entry_prefix(
    device: &Arc<dyn BlockDriver>,
    entries_lba: usize,
    entry_size: usize,
    index: usize,
    out: &mut [u8; GPT_ENTRY_PREFIX_LEN],
) -> bool {
    let Some(entry_offset) = entry_size.checked_mul(index) else {
        return false;
    };

    let mut copied = 0;
    while copied < out.len() {
        let absolute_offset = entry_offset + copied;
        let lba_offset = absolute_offset / 512;
        let Some(lba) = entries_lba.checked_add(lba_offset) else {
            return false;
        };
        if lba >= device.total_blocks() {
            return false;
        }

        let offset_in_block = absolute_offset % 512;
        let bytes_to_copy = min(out.len() - copied, 512 - offset_in_block);

        let mut sector = [0u8; 512];
        if !device.read_block(lba, &mut sector) {
            return false;
        }

        out[copied..copied + bytes_to_copy]
            .copy_from_slice(&sector[offset_in_block..offset_in_block + bytes_to_copy]);
        copied += bytes_to_copy;
    }

    true
}

fn le_u32(bytes: &[u8]) -> u32 {
    u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]])
}

fn le_u64(bytes: &[u8]) -> u64 {
    u64::from_le_bytes([
        bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
    ])
}
