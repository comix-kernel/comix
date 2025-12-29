//! LoongArch64 内核任务模块

use core::arch::global_asm;

pub mod context;
pub mod task;

global_asm!(include_str!("switch.S"));

// 上下文切换函数
unsafe extern "C" {
    pub fn switch(old: *mut Context, new: *const Context);
}

/// CPU 相关
pub mod cpu {
    /// 获取当前 Hart ID（当前仅单核）
    pub fn hart_id() -> usize {
        0
    }

    /// 获取 CPU ID（别名）
    pub fn cpu_id() -> usize {
        hart_id()
    }
}

pub use context::TaskContext;

/// Context 类型别名（用于兼容）
pub type Context = TaskContext;
