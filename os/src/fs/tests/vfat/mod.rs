use crate::device::block::BlockDriver;
use crate::device::ram_disk::RamDisk;
use crate::fs::vfat::adapter::{FatBlockDevice, VfatIoError};
use crate::{kassert, test_case};
use alloc::sync::Arc;
use alloc::vec;
use fatfs::{Read, Seek, Write};

fn patterned_disk(size: usize, block_size: usize) -> Arc<RamDisk> {
    let mut data = vec![0u8; size];
    for (index, byte) in data.iter_mut().enumerate() {
        *byte = (index % 251) as u8;
    }
    RamDisk::from_bytes(data, block_size, 0)
}

test_case!(test_vfat_adapter_reads_unaligned_across_blocks, {
    let disk = patterned_disk(1024, 512);
    let mut device = FatBlockDevice::new(disk).unwrap();

    device.seek(fatfs::SeekFrom::Start(510)).unwrap();
    let mut buf = [0u8; 8];
    let read = device.read(&mut buf).unwrap();

    kassert!(read == 8);
    kassert!(&buf == &[8, 9, 10, 11, 12, 13, 14, 15]);
    kassert!(device.position() == 518);
});

test_case!(test_vfat_adapter_write_preserves_partial_blocks, {
    let disk = patterned_disk(1024, 512);
    let mut device = FatBlockDevice::new(disk.clone()).unwrap();

    device.seek(fatfs::SeekFrom::Start(510)).unwrap();
    let written = device.write(&[0xAA, 0xBB, 0xCC, 0xDD]).unwrap();

    kassert!(written == 4);
    let raw = disk.raw_data();
    kassert!(raw[509] == (509 % 251) as u8);
    kassert!(raw[510] == 0xAA);
    kassert!(raw[511] == 0xBB);
    kassert!(raw[512] == 0xCC);
    kassert!(raw[513] == 0xDD);
    kassert!(raw[514] == (514 % 251) as u8);
});

test_case!(test_vfat_adapter_writes_full_aligned_block, {
    let disk = patterned_disk(1024, 512);
    let mut device = FatBlockDevice::new(disk.clone()).unwrap();
    let block = [0x5Au8; 512];

    device.seek(fatfs::SeekFrom::Start(512)).unwrap();
    let written = device.write(&block).unwrap();

    kassert!(written == 512);
    let mut read_back = [0u8; 512];
    kassert!(disk.read_block(1, &mut read_back));
    kassert!(read_back == block);
});

test_case!(test_vfat_adapter_read_stops_at_device_end, {
    let disk = patterned_disk(1024, 512);
    let mut device = FatBlockDevice::new(disk).unwrap();

    device.seek(fatfs::SeekFrom::Start(1020)).unwrap();
    let mut buf = [0u8; 8];
    let read = device.read(&mut buf).unwrap();

    kassert!(read == 4);
    kassert!(device.position() == 1024);
});

test_case!(test_vfat_adapter_rejects_out_of_bounds_write, {
    let disk = patterned_disk(1024, 512);
    let mut device = FatBlockDevice::new(disk).unwrap();

    device.seek(fatfs::SeekFrom::Start(1020)).unwrap();
    let result = device.write(&[1, 2, 3, 4, 5]);

    kassert!(result == Err(VfatIoError::OutOfBounds));
    kassert!(device.position() == 1020);
});

test_case!(test_vfat_adapter_rejects_invalid_seek, {
    let disk = patterned_disk(1024, 512);
    let mut device = FatBlockDevice::new(disk).unwrap();

    kassert!(device.seek(fatfs::SeekFrom::End(1)) == Err(VfatIoError::OutOfBounds));
    kassert!(device.seek(fatfs::SeekFrom::Current(-1)) == Err(VfatIoError::OutOfBounds));
    kassert!(device.position() == 0);
});
