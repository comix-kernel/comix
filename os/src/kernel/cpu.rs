//! CPU 相关模块
//!
//! 包含 CPU 结构体及其相关操作
use alloc::sync::Arc;
use alloc::vec::Vec;

use crate::mm::activate;
use crate::{kernel::task::SharedTask, mm::memory_space::MemorySpace, sync::SpinLock};
use lazy_static::lazy_static;

pub static mut NUM_CPU: usize = 1;
pub static mut CLOCK_FREQ: usize = 12_500_000;

lazy_static! {
    pub static ref CPUS: Vec<SpinLock<Cpu>> = {
        let num_cpu = unsafe { NUM_CPU };
        let mut cpus = Vec::with_capacity(num_cpu);
        for _ in 0..num_cpu {
            cpus.push(SpinLock::new(Cpu::new()));
        }
        cpus
    };
}

/// CPU 结构体
pub struct Cpu {
    /// 任务上下文
    /// 用于在调度器中保存和恢复 CPU 寄存器状态
    /// 当前运行的任务
    pub current_task: Option<SharedTask>,
    /// 当前使用的内存空间
    /// 对于内核线程，其本身相应字段为 None，因而使用上一个任务的内存空间
    pub current_memory_space: Option<Arc<SpinLock<MemorySpace>>>,
}

impl Cpu {
    /// 创建一个新的 CPU 实例
    pub fn new() -> Self {
        Cpu {
            current_task: None,
            current_memory_space: None,
        }
    }

    /// 切换当前任务
    /// # 参数
    /// * `task` - 要切换到的任务
    pub fn switch_task(&mut self, task: SharedTask) {
        self.current_task = Some(task.clone());
        if !task.lock().is_kernel_thread() {
            self.current_memory_space = task.lock().memory_space.clone();
            activate(
                self.current_memory_space
                    .as_ref()
                    .unwrap()
                    .lock()
                    .root_ppn(),
            );
        }
    }

    /// 切换当前内存空间
    /// # 参数
    /// * `space` - 要切换到的内存空间
    pub fn switch_space(&mut self, space: Arc<SpinLock<MemorySpace>>) {
        self.current_memory_space = Some(space.clone());
        activate(space.lock().root_ppn());
    }
}

/// 获取当前 CPU 的引用
pub fn current_cpu() -> &'static SpinLock<Cpu> {
    let cpu_id = crate::arch::kernel::cpu::cpu_id();
    &CPUS[cpu_id]
}
