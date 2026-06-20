use crate::device::block::BlockDriver;
use crate::device::block::partition::{
    PartitionBlockDevice, discover_partitions, read_mbr_partitions,
};
use crate::device::ram_disk::RamDisk;
use crate::{kassert, test_case};
use alloc::string::ToString;
use alloc::sync::Arc;

fn write_mbr_entry(
    sector: &mut [u8; 512],
    index: usize,
    type_code: u8,
    start_lba: u32,
    sector_count: u32,
) {
    let offset = 446 + index * 16;
    sector[offset + 4] = type_code;
    sector[offset + 8..offset + 12].copy_from_slice(&start_lba.to_le_bytes());
    sector[offset + 12..offset + 16].copy_from_slice(&sector_count.to_le_bytes());
}

test_case!(test_mbr_partition_discovery, {
    let disk = RamDisk::new(16 * 512, 512, 0);
    let mut mbr = [0u8; 512];
    write_mbr_entry(&mut mbr, 1, 0x0c, 4, 8);
    mbr[510] = 0x55;
    mbr[511] = 0xAA;
    kassert!(disk.write_block(0, &mbr));

    let disk: Arc<dyn BlockDriver> = disk;
    let partitions = read_mbr_partitions(&disk);
    kassert!(partitions[0].is_none());
    let second = partitions[1].unwrap();
    kassert!(second.number == 2);
    kassert!(second.type_code == 0x0c);
    kassert!(second.start_lba == 4);
    kassert!(second.sector_count == 8);
});

test_case!(test_partition_block_device_maps_block_range, {
    let disk = RamDisk::new(16 * 512, 512, 0);
    let disk_driver: Arc<dyn BlockDriver> = disk.clone();
    let partition =
        PartitionBlockDevice::new(disk_driver, "vda2".to_string(), 4, 8).unwrap();

    let pattern = [0x5Au8; 512];
    kassert!(partition.write_block(0, &pattern));

    let mut whole_disk_block = [0u8; 512];
    kassert!(disk.read_block(4, &mut whole_disk_block));
    kassert!(whole_disk_block[0] == 0x5A);
    kassert!(whole_disk_block[511] == 0x5A);
    kassert!(partition.total_blocks() == 8);

    let out_of_range = [0u8; 512];
    kassert!(!partition.write_block(8, &out_of_range));
});

test_case!(test_gpt_partition_discovery, {
    let disk = RamDisk::new(64 * 512, 512, 0);

    let mut mbr = [0u8; 512];
    write_mbr_entry(&mut mbr, 0, 0xEE, 1, 63);
    mbr[510] = 0x55;
    mbr[511] = 0xAA;
    kassert!(disk.write_block(0, &mbr));

    let mut header = [0u8; 512];
    header[0..8].copy_from_slice(b"EFI PART");
    header[72..80].copy_from_slice(&2u64.to_le_bytes());
    header[80..84].copy_from_slice(&4u32.to_le_bytes());
    header[84..88].copy_from_slice(&128u32.to_le_bytes());
    kassert!(disk.write_block(1, &header));

    let mut entries = [0u8; 512];
    let second_entry = 128;
    entries[second_entry] = 1;
    entries[second_entry + 32..second_entry + 40].copy_from_slice(&10u64.to_le_bytes());
    entries[second_entry + 40..second_entry + 48].copy_from_slice(&20u64.to_le_bytes());
    kassert!(disk.write_block(2, &entries));

    let disk: Arc<dyn BlockDriver> = disk;
    let partitions = discover_partitions(&disk);
    kassert!(partitions.len() == 1);
    kassert!(partitions[0].number == 2);
    kassert!(partitions[0].start_lba == 10);
    kassert!(partitions[0].sector_count == 11);
});
