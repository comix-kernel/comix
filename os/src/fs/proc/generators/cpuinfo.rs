use alloc::vec::Vec;

use crate::fs::proc::inode::ContentGenerator;
use crate::vfs::FsError;

pub struct CpuinfoGenerator;

impl ContentGenerator for CpuinfoGenerator {
    fn generate(&self) -> Result<Vec<u8>, FsError> {
        Ok(proc_cpuinfo())
    }
}

#[cfg(target_arch = "riscv64")]
fn proc_cpuinfo() -> Vec<u8> {
    b"processor\t: 0\n\
hart\t\t: 0\n\
isa\t\t: rv64imafdcsu\n\
mmu\t\t: sv39\n\
uarch\t\t: qemu,virt\n\n"
        .to_vec()
}

#[cfg(target_arch = "loongarch64")]
fn proc_cpuinfo() -> Vec<u8> {
    b"processor\t: 0\n\
arch\t\t: loongarch64\n\n"
        .to_vec()
}

#[cfg(not(any(target_arch = "riscv64", target_arch = "loongarch64")))]
fn proc_cpuinfo() -> Vec<u8> {
    b"processor\t: 0\narch\t\t: mock\n\n".to_vec()
}
