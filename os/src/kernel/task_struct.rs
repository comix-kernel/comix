use crate::arch::kernel::context::Context;
use crate::arch::trap::kerneltrap::TrapFrame;

/// 关于任务的资源信息
/// 存放与进程资源、内存管理、I/O 权限、用户 ID 等相关的、相对稳定或低频访问的数据。
/// 主要由内存管理子系统和权限管理子系统使用。
#[allow(dead_code)]
pub struct TaskStruct {
    /// 内核栈基址
    kstack_base: usize,
    /// 中断上下文。指向当前任务内核栈上的 TrapFrame，仅在任务被中断时有效。
    trap_frame_ptr: *mut TrapFrame,
    /// 任务上下文，用于任务切换
    context: Context,
    /// 父任务的id
    parient_tid: usize,
    /// 退出码
    exit_code: isize,
    // TODO: 由于部分相关子系统尚未实现，暂时留空
}
