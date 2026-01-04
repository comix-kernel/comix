//! RISC-V 架构信息（供 /proc 等使用）

use alloc::vec::Vec;

/// 生成 `/proc/cpuinfo` 内容。
pub fn proc_cpuinfo() -> Vec<u8> {
    // 目前为静态内容；后续可从设备树/CPU 特性寄存器动态填充。
    b"processor\t: 0\n\
hart\t\t: 0\n\
isa\t\t: rv64imafdcsu\n\
mmu\t\t: sv39\n\
uarch\t\t: qemu,virt\n\n"
        .to_vec()
}
