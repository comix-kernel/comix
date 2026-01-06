//! LoongArch64 架构信息（供 /proc 等使用）

use alloc::vec::Vec;

/// 生成 `/proc/cpuinfo` 内容。
pub fn proc_cpuinfo() -> Vec<u8> {
    // TODO: 从硬件/固件读取更完整的信息。
    b"processor\t: 0\n\
arch\t\t: loongarch64\n\n"
        .to_vec()
}
