//! LoongArch64 内核任务模块

use core::arch::global_asm;

use crate::arch::kernel::context::Context;

pub mod context;
pub mod task;

global_asm!(include_str!("switch.S"));

unsafe extern "C" {
    /// 上下文切换函数
    ///
    /// 保存当前任务的寄存器状态到 old 指向的 context 结构体中，
    /// 然后从 new 指向的 context 结构体中恢复寄存器状态，切换到新任务执行。
    pub unsafe fn switch(old: *mut Context, new: *const Context);
}

/// CPU 相关
pub mod cpu {
    /// 获取当前 Hart ID
    pub fn hart_id() -> usize {
        0
    }

    /// 获取 CPU ID（别名）
    pub fn cpu_id() -> usize {
        hart_id()
    }
}

pub use context::TaskContext;
