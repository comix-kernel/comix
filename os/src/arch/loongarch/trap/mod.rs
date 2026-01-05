//! LoongArch64 陷阱/异常处理模块

mod trap_handler;
pub mod trap_frame;

use core::arch::global_asm;

use loongArch64::register::eentry;

pub use trap_frame::TrapFrame;

global_asm!(include_str!("trap_entry.S"));

/// 用户内存访问守卫
pub struct SumGuard;

impl SumGuard {
    /// 创建新的守卫，允许访问用户内存
    pub fn new() -> Self {
        // TODO: 实现 LoongArch 用户内存访问控制
        Self
    }
}

impl Default for SumGuard {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for SumGuard {
    fn drop(&mut self) {
        // TODO: 恢复用户内存访问控制
    }
}

/// 初始化启动阶段陷阱处理
pub fn init_boot_trap() {
    set_trap_entry();
}

/// 初始化陷阱处理
pub fn init() {
    set_trap_entry();
}

/// 恢复陷阱帧
pub unsafe fn restore(tf: &TrapFrame) -> ! {
    unsafe { __restore(tf) }
}

/// 获取信号返回 trampoline 地址
pub fn sigreturn_trampoline_address() -> usize {
    // TODO: 实现信号返回 trampoline
    0
}

/// 设置当前任务的 TrapFrame 指针（使用 CSR.SAVE0）
pub fn set_trap_frame_ptr(ptr: usize) {
    unsafe {
        core::arch::asm!("csrwr {0}, 0x30", in(reg) ptr, options(nostack, preserves_flags));
    }
}

fn set_trap_entry() {
    // EENTRY 要求 4KB 对齐，trap_entry 在汇编中对齐
    eentry::set_eentry(trap_entry as usize);
}

unsafe extern "C" {
    unsafe fn trap_entry();
    unsafe fn __restore(trap_frame: &TrapFrame) -> !;
}
