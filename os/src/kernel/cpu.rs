use alloc::sync::Arc;

use crate::{kernel::task::SharedTask, mm::memory_space::MemorySpace, sync::SpinLock};

/// CPU 结构体
#[allow(dead_code)]
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
    pub fn new() -> Self {
        Cpu {
            current_task: None,
            cur_memory_space: None,
        }
    }
}

pub fn current_cpu() -> &'static SpinLock<Cpu> {
    let cpu_id = crate::arch::kernel::cpu::cpu_id();
    &crate::kernel::CPUS[cpu_id]
}
