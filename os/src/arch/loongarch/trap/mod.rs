//! LoongArch64 陷阱/异常处理模块

use core::arch::global_asm;

mod sum_guard;
pub mod trap_frame;
mod trap_handler;

pub use sum_guard::SumGuard;
pub use trap_frame::TrapFrame;

// 汇编入口与恢复例程
global_asm!(include_str!("trap_entry.S"));
global_asm!(include_str!("sigreturn.S"));

/// 初始化启动阶段陷阱处理
pub fn init_boot_trap() {
    trap_handler::install_boot_trap();
}

/// 初始化陷阱处理
pub fn init() {
    trap_handler::install_runtime_trap();
}

/// 恢复陷阱帧上下文并返回
pub fn restore(tf: &TrapFrame) {
    trap_handler::restore_context(tf)
}

/// 获取信号返回 trampoline 地址
pub fn sigreturn_trampoline_address() -> usize {
    crate::config::USER_SIGRETURN_TRAMPOLINE
}

unsafe extern "C" {
    fn __sigreturn_trampoline();
    fn __sigreturn_trampoline_end();
}

/// Kernel-side instruction bytes for the rt_sigreturn trampoline.
///
/// These bytes are copied into a userspace RX page at `sigreturn_trampoline_address()`.
pub fn kernel_sigreturn_trampoline_bytes() -> &'static [u8] {
    let start = __sigreturn_trampoline as usize;
    let end = __sigreturn_trampoline_end as usize;
    let len = end.saturating_sub(start);
    unsafe { core::slice::from_raw_parts(start as *const u8, len) }
}

/// 设置 TrapFrame 中与当前 CPU 相关的字段（占位符）。
///
/// LoongArch 端 TrapFrame 当前不携带类似 RISC-V 的 `cpu_ptr` 字段；
/// 为了让通用任务初始化/迁移逻辑保持无条件编译，这里提供 no-op 接口。
#[inline]
pub unsafe fn set_trap_frame_cpu_ptr(_trap_frame_ptr: *mut TrapFrame, _cpu_ptr: usize) {}
