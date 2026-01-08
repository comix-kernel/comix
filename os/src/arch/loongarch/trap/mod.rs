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
    __sigreturn_trampoline as usize
}

unsafe extern "C" {
    fn __sigreturn_trampoline();
}

/// 设置 TrapFrame 中与当前 CPU 相关的字段。
#[inline]
pub unsafe fn set_trap_frame_cpu_ptr(trap_frame_ptr: *mut TrapFrame, cpu_ptr: usize) {
    if let Some(tf) = trap_frame_ptr.as_mut() {
        tf.cpu_ptr = cpu_ptr;
    }
}
