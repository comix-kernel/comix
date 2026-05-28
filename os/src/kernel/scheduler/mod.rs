//！ 调度器模块
//!
//！ 定义了调度器接口和相关功能
mod rr_scheduler;
mod task_queue;
mod wait_queue;

use core::sync::atomic::{AtomicUsize, Ordering};

use crate::{
    arch::Arch,
    arch::kernel::context::Context,
    config::MAX_CPU_COUNT,
    kernel::{TaskState, TaskStruct, scheduler::rr_scheduler::RRScheduler, task::SharedTask},
    sync::SpinLock,
};

pub use task_queue::TaskQueue;
pub use wait_queue::WaitQueue;

/// Per-CPU 调度器数组
/// 每个 CPU 拥有独立的运行队列和调度器实例
static SCHEDULERS: [SpinLock<RRScheduler>; MAX_CPU_COUNT] =
    [const { SpinLock::new(RRScheduler::empty()) }; MAX_CPU_COUNT];

/// 负载均衡计数器
/// 用于简单轮转选择目标 CPU
static NEXT_CPU: AtomicUsize = AtomicUsize::new(0);

/// 上下文切换计划结构体
pub(crate) struct SwitchPlan {
    pub old: *mut Context,
    pub new: *const Context,
}

/// 调度器接口定义
/// 调度器负责决策和准备下一个任务的运行。
/// 具体来说，它负责以下几项工作：
/// 1. 决策： 根据优先级、时间片、调度策略等算法，从运行队列中选择下一个要执行的任务。
/// 2. 队列维护： 维护任务的运行队列（Run Queue）。
pub trait Scheduler {
    /// 构造函数
    fn new() -> Self;
    /// 添加任务到调度器
    /// 参数:
    /// * `task`: 需要添加的任务
    fn add_task(&mut self, task: SharedTask);
    /// 选择下一个要运行的任务
    /// # 返回值
    /// 如果要切换到下一个任务，返回切换计划 SwitchPlan；否则返回 None
    fn next_task(&mut self) -> Option<SwitchPlan>;
    /// 任务阻塞
    /// 修改任务状态并从运行队列中移除
    /// 参数:
    /// * `task`: 需要阻塞的任务
    /// * `receive_signal`: 是否可被信号中断
    /// 注意: 该函数仅设置状态，不负责切换任务
    fn sleep_task(&mut self, task: SharedTask, receive_signal: bool);
    /// 唤醒任务
    /// 修改任务状态并将其添加到运行队列
    /// 参数:
    /// * `task`: 需要唤醒的任务
    fn wake_up(&mut self, task: SharedTask);
    /// 任务终止
    /// 修改任务状态并从调度器中移除
    /// 参数:
    /// * `task`: 需要终止的任务
    fn exit_task(&mut self, task: SharedTask);
    /// 原子地检查条件并睡眠
    ///
    /// 在持有调度器锁和 task 锁的情况下执行 `prepare` 闭包。
    /// 如果 `prepare` 返回 `true`（条件满足，不需要睡眠），
    /// 则任务保持唤醒并返回 `false`。
    /// 如果 `prepare` 返回 `false`，则将任务设为睡眠态并从运行队列中移除，
    /// 返回 `true`。
    fn sleep_task_prepare(
        &mut self,
        task: SharedTask,
        receive_signal: bool,
        prepare: impl FnOnce(&mut crate::kernel::TaskStruct) -> bool,
    ) -> bool;
}

/// 获取当前 CPU 的调度器
pub fn current_scheduler() -> &'static SpinLock<RRScheduler> {
    let cpu_id = crate::arch::cpu_id();
    &SCHEDULERS[cpu_id]
}

/// 获取指定 CPU 的调度器
pub fn scheduler_of(cpu_id: usize) -> &'static SpinLock<RRScheduler> {
    &SCHEDULERS[cpu_id]
}

/// 通过轮询方式为新任务选择一个目标 CPU。
pub fn pick_cpu() -> usize {
    let num_cpu = crate::kernel::num_cpu();
    NEXT_CPU.fetch_add(1, Ordering::Relaxed) % num_cpu
}

/// 执行一次调度操作，切换到下一个任务
pub fn schedule() {
    // 读取并禁用中断，保护整个调度过程，并在返回时恢复原状态
    let flags = crate::arch::disable_interrupts();

    // 快速路径：如果运行队列为空且当前任务仍是 Running，就无需进入调度器
    let should_try_switch = {
        let sched = current_scheduler().lock();
        let rq_empty = sched.is_empty();
        drop(sched);

        let cur_running = {
            let cpu = crate::kernel::current_cpu();
            cpu.current_task
                .as_ref()
                .map(|t| t.lock().state == crate::kernel::TaskState::Running)
                .unwrap_or(false)
        };
        !(rq_empty && cur_running)
    };

    if should_try_switch {
        let plan = {
            let mut sched = current_scheduler().lock();
            // NOTE: next_task 内部会更新 current_task 与 current_memory_space 并切换页表
            sched.next_task()
        }; // 调度器锁在这里释放

        if let Some(plan) = plan {
            // SAFETY: next_task 生成的上下文指针有效
            unsafe { crate::arch::ArchImpl::context_switch(plan.old, plan.new) };
            // 通常不会立即返回；返回时再继续当前上下文后续逻辑
        }
    }

    // 恢复进入前的中断状态
    crate::arch::restore_interrupt_state(flags);
}

/// 主动放弃 CPU
/// 切换到下一个任务
/// 如果调用该函数的任务仍可运行，将被放回运行队列末尾，等待下一次调度
pub fn yield_task() {
    schedule();
}

/// 任务阻塞
/// 修改任务状态并从运行队列中移除
/// 参数:
/// * `task`: 需要阻塞的任务
/// * `receive_signal`: 是否可被信号中断
/// 注意: 该函数仅设置状态，不负责切换任务
pub fn sleep_task(task: SharedTask, receive_signal: bool) {
    let cpu_id = {
        let t = task.lock();
        t.on_cpu.unwrap_or_else(crate::arch::cpu_id)
    };
    scheduler_of(cpu_id).lock().sleep_task(task, receive_signal);
}

/// 唤醒任务
/// 修改任务状态并将其添加到运行队列
/// 参数:
/// * `task`: 需要唤醒的任务
pub fn wake_up_task(task: SharedTask) {
    let target_cpu = pick_cpu();
    let current_cpu = crate::arch::cpu_id();
    let task_tid = { task.lock().tid };

    // 关键：多核下 wake 可能被重复触发（不同 CPU/不同事件源），必须做到“全局幂等”：
    // - 若任务已经是 Running（正在跑/已入队），则不要再次入队到其他 CPU 的运行队列
    // 否则同一任务可能被两个 CPU 同时调度运行，导致 TrapFrame/上下文被并发破坏（海森堡 panic/挂起）。
    let should_ipi;
    {
        let mut sched = scheduler_of(target_cpu).lock();

        // 用 task 锁串行化唤醒状态转换，避免跨 CPU 的“双重入队”
        {
            let mut t = task.lock();
            if t.state == TaskState::Running {
                return;
            }
            // Zombie/Stopped 不应被重新唤醒入队（保持现状，避免状态机混乱）
            if matches!(t.state, TaskState::Zombie | TaskState::Stopped) {
                return;
            }
            t.state = TaskState::Running;
            t.on_cpu = Some(target_cpu);
        }

        crate::pr_debug!(
            "[Scheduler] Waking up task {} on CPU {}",
            task_tid,
            target_cpu
        );
        sched.wake_up(task);
        should_ipi = target_cpu != current_cpu;
    }

    if should_ipi {
        crate::pr_debug!(
            "[Scheduler] Sending IPI from CPU {} to CPU {} for task {}",
            current_cpu,
            target_cpu,
            task_tid
        );
        crate::arch::send_reschedule_ipi(target_cpu);
    }
}

/// 任务终止
/// 修改任务状态并从调度器中移除
/// 参数:
/// * `task`: 需要终止的任务
pub fn exit_task(task: SharedTask) {
    let cpu_id = {
        let t = task.lock();
        t.on_cpu.unwrap_or_else(crate::arch::cpu_id)
    };
    scheduler_of(cpu_id).lock().exit_task(task);
}

/// 原子地检查条件并阻塞任务
///
/// 在持有调度器锁和 task 锁的情况下执行 `prepare` 闭包。
/// 如果 `prepare` 返回 `true`（条件满足），任务保持唤醒并返回 `false`。
/// 如果 `prepare` 返回 `false`，任务将被设为睡眠态并从运行队列移除，
/// 返回 `true`。
///
/// 这消除了 TOCTOU 竞态条件：条件检查和状态转换在锁内原子地完成。
pub fn sleep_task_prepare(
    task: SharedTask,
    receive_signal: bool,
    prepare: impl FnOnce(&mut TaskStruct) -> bool,
) -> bool {
    let cpu_id = {
        let t = task.lock();
        t.on_cpu.unwrap_or_else(crate::arch::cpu_id)
    };
    scheduler_of(cpu_id)
        .lock()
        .sleep_task_prepare(task, receive_signal, prepare)
}
