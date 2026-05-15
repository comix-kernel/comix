use alloc::vec::Vec;

use crate::fs::proc::inode::ContentGenerator;
use crate::vfs::FsError;

pub struct CpuinfoGenerator;

impl ContentGenerator for CpuinfoGenerator {
    fn generate(&self) -> Result<Vec<u8>, FsError> {
        Ok(crate::arch::abi::proc_cpuinfo_bytes().to_vec())
    }
}
