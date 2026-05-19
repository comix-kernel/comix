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

fn enable_base_fp(_hartid: usize) {
    // Enable base floating-point instructions (EUEN.FPE). Many LoongArch Linux-ABI
    // user programs are built with floating-point enabled and may execute FP
    // instructions very early during startup.
    loongArch64::register::euen::set_fpe(true);
}
