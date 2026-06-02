//! LoongArch64 架构相关的启动代码

use core::arch::global_asm;

global_asm!(include_str!("entry.S"));

use crate::kernel;

/// LoongArch 主核启动入口
pub fn main(hartid: usize) {
    let mut ops = kernel::boot::PrimaryBootOps::new("LoongArch", "CPU");
    ops.after_clear_bss = enable_base_fp;
    kernel::boot::run_primary_boot(hartid, ops);
}

// EUEN（扩展部件使能）CSR 编号
const CSR_EUEN: u32 = 0x2;
// EUEN.FPE：基础浮点指令使能位（bit 0）
const CSR_EUEN_FPE: usize = 0x1;

fn enable_base_fp(_hartid: usize) {
    // Enable base floating-point instructions (EUEN.FPE). Many LoongArch Linux-ABI
    // user programs are built with floating-point enabled and may execute FP
    // instructions very early during startup.
    //
    // 直接读改写 EUEN CSR，与本目录其余 CSR 操作风格一致，避免依赖外部 `loongArch64` crate。
    unsafe {
        let mut euen: usize;
        core::arch::asm!(
            "csrrd {val}, {euen}",
            val = out(reg) euen,
            euen = const CSR_EUEN,
            options(nostack, preserves_flags)
        );
        euen |= CSR_EUEN_FPE;
        core::arch::asm!(
            "csrwr {val}, {euen}",
            val = in(reg) euen,
            euen = const CSR_EUEN,
            options(nostack, preserves_flags)
        );
    }
}
