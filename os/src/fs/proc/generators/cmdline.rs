use alloc::vec::Vec;

use crate::fs::proc::inode::ContentGenerator;
use crate::vfs::FsError;

pub struct KernelCmdlineGenerator;

impl ContentGenerator for KernelCmdlineGenerator {
    fn generate(&self) -> Result<Vec<u8>, FsError> {
        let mut cmdline = crate::device::CMDLINE.read().clone().into_bytes();
        cmdline.push(b'\n');
        Ok(cmdline)
    }
}
