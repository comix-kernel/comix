//! LoongArch64 内核任务模块（存根）

pub mod context;
pub mod task;

/// 上下文切换模块
pub mod switch {
    use super::context::TaskContext;

    /// 切换上下文
    /// # Safety
    /// 直接操作栈和寄存器
    #[inline(always)]
    pub unsafe fn __switch(current_ctx: *mut TaskContext, next_ctx: *const TaskContext) {
        // TODO: 实现 LoongArch 上下文切换
        let _ = (current_ctx, next_ctx);
    }
}

/// 上下文切换函数（与 RISC-V 兼容的接口）
/// # Safety
/// 直接操作栈和寄存器
#[inline(always)]
pub unsafe fn switch(old: *mut Context, new: *const Context) {
    // SAFETY: 调用者负责确保上下文指针有效
    unsafe { switch::__switch(old, new) }
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

/// Context 类型别名（用于兼容）
pub type Context = TaskContext;
