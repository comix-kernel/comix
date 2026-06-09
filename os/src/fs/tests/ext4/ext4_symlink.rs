use super::*;
use crate::fs::ext4::Ext4Inode;
use crate::vfs::file_system::FileSystem;
use crate::vfs::inode::InodeType;
use crate::{kassert, test_case};
use alloc::string::String;

fn repeated_target(byte: u8, len: usize) -> String {
    String::from_utf8(alloc::vec![byte; len]).unwrap()
}

test_case!(test_ext4_fast_symlink_59_bytes, {
    let fs = create_test_ext4();
    let root = fs.root_inode();
    let target = repeated_target(b'f', 59);

    let link = root.symlink("fast59", &target).unwrap();
    let metadata = link.metadata().unwrap();

    kassert!(metadata.inode_type == InodeType::Symlink);
    kassert!(metadata.size == 59);
    kassert!(metadata.blocks == 0);
    kassert!(link.readlink().unwrap() == target);
});

test_case!(test_ext4_slow_symlink_60_bytes, {
    let fs = create_test_ext4();
    let root = fs.root_inode();
    let target = repeated_target(b's', 60);

    let link = root.symlink("slow60", &target).unwrap();
    let metadata = link.metadata().unwrap();

    kassert!(metadata.inode_type == InodeType::Symlink);
    kassert!(metadata.size == 60);
    kassert!(metadata.blocks > 0);
    kassert!(link.readlink().unwrap() == target);
});

test_case!(test_ext4_fast_symlink_ignores_blocks_count, {
    let fs = create_test_ext4();
    let root = fs.root_inode();
    let target = repeated_target(b'x', 59);

    let link = root.symlink("fast_with_blocks", &target).unwrap();
    let ext4_inode = link.downcast_ref::<Ext4Inode>().unwrap();
    ext4_inode.set_blocks_count_for_test(8);

    let metadata = link.metadata().unwrap();
    kassert!(metadata.blocks > 0);
    kassert!(link.readlink().unwrap() == target);
});
