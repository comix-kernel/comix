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

    /// 在切换到指定任务后执行的架构相关收尾工作。
    ///
    /// LoongArch 目前尚未实现 trap/上下文切换，因此此处为 no-op。
    pub fn on_task_switch(_trap_frame_ptr: usize, _cpu_ptr: usize) {}
}

pub use context::TaskContext;

/// Context 类型别名（用于兼容）
pub type Context = TaskContext;
