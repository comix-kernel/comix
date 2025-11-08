//! CPU 相关模块
//! 包含 CPU 结构体及其相关操作
use alloc::sync::Arc;

use crate::config::NUM_CPU;
use crate::{kernel::task::SharedTask, mm::memory_space::MemorySpace, sync::SpinLock};
use core::array;
use lazy_static::lazy_static;

lazy_static! {
    pub static ref CPUS: [SpinLock<Cpu>; NUM_CPU] = array::from_fn(|_| SpinLock::new(Cpu::new()));
}

/// CPU 结构体
pub struct Cpu {
    /// 任务上下文
    /// 用于在调度器中保存和恢复 CPU 寄存器状态
    /// 当前运行的任务
    pub current_task: Option<SharedTask>,
    /// 当前使用的内存空间
    /// 对于内核线程，其本身相应字段为 None，因而使用上一个任务的内存空间
    pub cur_memory_space: Option<Arc<MemorySpace>>,
}

impl Cpu {
    /// 创建一个新的 CPU 实例
    pub fn new() -> Self {
        Cpu {
            current_task: None,
            cur_memory_space: None,
        }
    }
}

/// 获取当前 CPU 的引用
pub fn current_cpu() -> &'static SpinLock<Cpu> {
    let cpu_id = crate::arch::kernel::cpu::cpu_id();
    &CPUS[cpu_id]
}
