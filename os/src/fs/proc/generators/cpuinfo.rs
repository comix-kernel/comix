use alloc::format;
use alloc::vec::Vec;

use crate::fs::proc::inode::ContentGenerator;
use crate::vfs::FsError;

pub struct CpuinfoGenerator;

impl ContentGenerator for CpuinfoGenerator {
    fn generate(&self) -> Result<Vec<u8>, FsError> {
        #[cfg(target_arch = "riscv64")]
        {
            let content = format!(
                "processor\t: 0\n\
                 hart\t\t: 0\n\
                 isa\t\t: rv64imafdcsu\n\
                 mmu\t\t: sv39\n\
                 uarch\t\t: qemu,virt\n\n"
            );
            Ok(content.into_bytes())
        }

        #[cfg(not(target_arch = "riscv64"))]
        {
            Ok(b"processor\t: 0\n\n".to_vec())
        }
    }
}
