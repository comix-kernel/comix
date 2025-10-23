use crate::{arch::kernel::context::Context, kernel::task::Task};

/// CPU 结构体
pub struct Cpu {
    /// 任务上下文
    /// 用于在调度器中保存和恢复 CPU 寄存器状态
    pub context: Context,
    /// 当前运行的任务
    pub current_task: Option<Task>,
}

impl Cpu {
    pub fn new() -> Self {
        Cpu {
            context: Context::zero_init(),
            current_task: None,
        }
    }
}

pub fn current_cpu() -> &'static Cpu {
    let cpu_id = crate::arch::cpu::cpu_id();
    &crate::kernel::CPUS[cpu_id]
}
