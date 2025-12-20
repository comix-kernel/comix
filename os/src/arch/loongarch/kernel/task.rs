//! LoongArch64 任务相关（存根）

use super::context::TaskContext;
use crate::mm::frame_allocator::FrameTracker;

/// 初始化内核任务上下文
pub fn init_kernel_task_context(context: &mut TaskContext, entry: usize, kstack: usize) {
    context.ra = entry;
    context.sp = kstack;
}

/// 初始化 fork 后的上下文
pub fn init_fork_context(
    context: &mut TaskContext,
    kstack: usize,
    trap_frame_tracker: &FrameTracker,
) {
    // TODO: 实现
    context.sp = kstack;
    let _ = trap_frame_tracker;
}

/// 设置用户栈布局
/// 返回 (new_sp, argc, argv_ptr, envp_ptr)
pub fn setup_stack_layout(
    _sp: usize,
    _argv: &[&str],
    _envp: &[&str],
    _phdr_addr: usize,
    _phnum: usize,
    _phent: usize,
    _entry_point: usize,
) -> (usize, usize, usize, usize) {
    // TODO: 实现 LoongArch 栈布局
    // 返回 (new_sp, argc, argv_ptr, envp_ptr)
    (0, 0, 0, 0)
}
