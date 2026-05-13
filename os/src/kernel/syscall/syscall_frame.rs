//! 系统调用帧抽象
//!
//! 为不同架构的 TrapFrame 提供统一的寄存器访问接口，
//! 使得 `dispatch_syscall` 和 `impl_syscall!` 宏可以架构无关。

/// TrapFrame 中系统调用相关寄存器的统一访问 trait。
pub trait SyscallFrame {
    fn syscall_id(&self) -> usize;
    fn arg0(&self) -> usize;
    fn arg1(&self) -> usize;
    fn arg2(&self) -> usize;
    fn arg3(&self) -> usize;
    fn arg4(&self) -> usize;
    fn arg5(&self) -> usize;
    fn set_ret(&mut self, val: usize);
}
