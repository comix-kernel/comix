//! CPU 相关模块
//!
//! 包含 CPU 结构体及其相关操作
use alloc::sync::Arc;

use crate::mm::activate;
use crate::{
    kernel::task::SharedTask,
    mm::memory_space::MemorySpace,
    sync::{PerCpu, SpinLock},
};
use lazy_static::lazy_static;

pub static mut NUM_CPU: usize = 1;
pub static mut CLOCK_FREQ: usize = 12_500_000;

lazy_static! {
    /// Per-CPU 数据: 每个 CPU 的状态
    ///
    /// 使用 PerCpu 容器自动实现缓存行对齐，避免伪共享。
    /// 每个 CPU 只访问自己的 Cpu 实例，不需要锁保护。
    ///
    /// 注意：使用 MAX_CPU_COUNT 而不是 NUM_CPU，避免 lazy_static 初始化时机问题
    pub static ref CPUS: PerCpu<Cpu> = {
        use crate::config::MAX_CPU_COUNT;
        PerCpu::new_with_id_and_count(MAX_CPU_COUNT, |cpu_id| Cpu::new_with_id(cpu_id))
    };
}

/// CPU 结构体
#[repr(C)]
pub struct Cpu {
    /// CPU ID (必须是第一个字段,用于快速访问)
    pub cpu_id: usize,
    /// 当前运行的任务
    pub current_task: Option<SharedTask>,
    /// 当前使用的内存空间
    /// 对于内核线程，其本身相应字段为 None，因而使用上一个任务的内存空间
    pub current_memory_space: Option<Arc<SpinLock<MemorySpace>>>,
    /// 本 CPU 的 idle 任务（永远可用的兜底任务）
    /// 不在运行队列中，当没有可运行任务时切换到它并在其中 WFI。
    pub idle_task: Option<SharedTask>,
}

impl Cpu {
    /// 创建一个新的 CPU 实例
    pub fn new() -> Self {
        Cpu {
            cpu_id: 0,
            current_task: None,
            current_memory_space: None,
            idle_task: None,
        }
    }

    /// 创建一个新的 CPU 实例 (指定 CPU ID)
    pub fn new_with_id(cpu_id: usize) -> Self {
        Cpu {
            cpu_id,
            current_task: None,
            current_memory_space: None,
            idle_task: None,
        }
    }

    /// 切换当前任务
    /// # 参数
    /// * `task` - 要切换到的任务
    pub fn switch_task(&mut self, task: SharedTask) {
        // 切换当前任务，并在必要时切换到其地址空间
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

        // 同步 TrapFrame 的 cpu_ptr 指向当前 CPU，确保多核迁移后 trap_entry 恢复正确的 tp
        // 说明：trap_entry 会从 TrapFrame.cpu_ptr 恢复 tp，如果任务在不同 CPU 之间迁移，
        // 需要将该字段更新为当前 CPU 的地址，否则进入内核后会把 tp 设置为错误的 CPU，
        // 导致 current_cpu()/per-CPU 数据读取混乱乃至崩溃。
        let tf_usize = {
            use core::sync::atomic::Ordering;
            task.lock().trap_frame_ptr.load(Ordering::SeqCst) as usize
        };
        crate::arch::kernel::cpu::on_task_switch(tf_usize, self as *const _ as usize);
    }

    /// 切换当前内存空间
    /// # 参数
    /// * `space` - 要切换到的内存空间
    pub fn switch_space(&mut self, space: Arc<SpinLock<MemorySpace>>) {
        self.current_memory_space = Some(space.clone());
        activate(space.lock().root_ppn());
    }
}

/// 获取当前 CPU 的引用 (可变)
///
/// # Safety
///
/// 调用者必须确保在访问期间禁用抢占 (防止任务迁移到其他 CPU)。
#[inline]
pub fn current_cpu() -> &'static mut Cpu {
    CPUS.get_mut()
}

/// 获取指定 CPU 的引用 (只读)
///
/// 用于跨核访问，例如负载均衡时查看其他 CPU 的任务。
#[inline]
pub fn cpu_of(cpu_id: usize) -> &'static Cpu {
    CPUS.get_of(cpu_id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{kassert, test_case};

    /// 测试 CPUS 初始化
    test_case!(test_cpus_initialization, {
        let num_cpu = unsafe { NUM_CPU };
        for cpu_id in 0..num_cpu {
            let cpu = CPUS.get_of(cpu_id);
            kassert!(cpu.cpu_id == cpu_id);
        }
    });

    /// 测试 cpu_id() 函数
    test_case!(test_cpu_id, {
        use crate::arch::kernel::cpu::cpu_id;
        use crate::sync::PreemptGuard;

        let _guard = PreemptGuard::new();
        let id = cpu_id();
        kassert!(id < unsafe { NUM_CPU });
    });

    /// 测试 current_cpu() 函数
    test_case!(test_current_cpu, {
        use crate::sync::PreemptGuard;

        let _guard = PreemptGuard::new();
        let cpu = current_cpu();
        kassert!(cpu.cpu_id < unsafe { NUM_CPU });
    });

    /// 测试 cpu_of() 函数
    test_case!(test_cpu_of, {
        let cpu0 = cpu_of(0);
        kassert!(cpu0.cpu_id == 0);

        let num_cpu = unsafe { NUM_CPU };
        if num_cpu > 1 {
            let cpu1 = cpu_of(1);
            kassert!(cpu1.cpu_id == 1);
        }
    });

    /// 测试 PerCpu 数据独立性（多核场景）
    test_case!(test_per_cpu_independence, {
        use crate::sync::{PerCpu, PreemptGuard};
        use core::sync::atomic::{AtomicUsize, Ordering};

        let per_cpu = PerCpu::new(|| AtomicUsize::new(0));

        // 在当前 CPU 上修改值
        {
            let _guard = PreemptGuard::new();
            let counter = per_cpu.get();
            counter.store(100, Ordering::Relaxed);
        }

        // 验证当前 CPU 的值
        {
            let _guard = PreemptGuard::new();
            let counter = per_cpu.get();
            kassert!(counter.load(Ordering::Relaxed) == 100);
        }

        // 验证其他 CPU 的值仍然是初始值
        let num_cpu = unsafe { NUM_CPU };
        let current_id = {
            let _guard = PreemptGuard::new();
            crate::arch::kernel::cpu::cpu_id()
        };

        for cpu_id in 0..num_cpu {
            if cpu_id != current_id {
                let counter = per_cpu.get_of(cpu_id);
                kassert!(counter.load(Ordering::Relaxed) == 0);
            }
        }
    });

    /// 测试 PerCpu 的 new_with_id 初始化
    test_case!(test_per_cpu_with_id, {
        use crate::sync::PerCpu;

        let per_cpu = PerCpu::new_with_id(|cpu_id| cpu_id * 10);

        let num_cpu = unsafe { NUM_CPU };
        for cpu_id in 0..num_cpu {
            let value = per_cpu.get_of(cpu_id);
            kassert!(*value == cpu_id * 10);
        }
    });
}
