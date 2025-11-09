//! RISC-V 架构相关的内核模块
use core::arch::global_asm;

use crate::arch::kernel::context::Context;

pub mod context;
pub mod cpu;
pub mod task;

global_asm!(include_str!("switch.S"));

unsafe extern "C" {
    /// 上下文切换函数
    ///
    /// 保存当前任务的寄存器状态到 old 指向的 context 结构体中，
    /// 然后从 new 指向的 context 结构体中恢复寄存器状态，切换到新任务执行。
    pub unsafe fn switch(old: *mut Context, new: *const Context);
}
