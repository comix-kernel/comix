use alloc::sync::Arc;

use crate::{
    arch::kernel::context::Context,
    kernel::{
        TaskState,
        cpu::current_cpu,
        scheduler::{Scheduler, SwitchPlan, schedule, task_queue::TaskQueue},
        task::SharedTask,
    },
};

#[cfg(not(test))]
const DEFAULT_TIME_SLICE: usize = 5; // 默认时间片长度

#[cfg(test)]
const DEFAULT_TIME_SLICE: usize = usize::MAX; // HACK: 测试时禁用调度

/// 简单的轮转调度器实现
/// 每个任务按顺序轮流获得 CPU 时间片
/// 约束：
/// 1. 要求开始调度后任何时刻，至少有一个任务处于运行状态
// XXX: 现在的实现是单核的。且没有支持内核抢占。
pub struct RRScheduler {
    // 运行队列
    run_queue: TaskQueue,
    // 等待队列
    wait_queue: TaskQueue,
    // 时间片长度（以时钟中断滴答数为单位）
    time_slice: usize,
    // 当前时间片剩余时间
    current_slice: usize,
}

impl RRScheduler {
    /// 更新当前时间片计数器
    /// # 返回值
    /// 如果时间片用尽，返回 true；否则返回 false
    pub fn update_time_slice(&mut self) -> bool {
        if self.current_slice > 0 {
            self.current_slice -= 1;
        }
        if self.current_slice == 0 {
            self.current_slice = self.time_slice;
            return true;
        }
        false
    }
}

impl Scheduler for RRScheduler {
    fn new() -> Self {
        RRScheduler {
            run_queue: TaskQueue::new(),
            wait_queue: TaskQueue::new(),
            time_slice: DEFAULT_TIME_SLICE,
            current_slice: DEFAULT_TIME_SLICE,
        }
    }

    fn prepare_switch(&mut self) -> Option<SwitchPlan> {
        // 取出当前任务，避免在下面赋值时被 Drop 掉
        let prev_task_opt = current_cpu().lock().current_task.take();

        // 选择下一个可运行任务（或返回/转 idle）
        let next_task = match self.next_task() {
            Some(t) => t,
            None => {
                // 没有可运行任务：恢复 current 并返回
                current_cpu().lock().current_task = prev_task_opt;
                return None;
            }
        };

        // 准备 new 上下文指针（短作用域锁）
        let new_ctx_ptr: *const Context = {
            let g = next_task.lock();
            &g.context as *const _
        };

        // 准备 old 上下文指针
        let old_ctx_ptr: *mut Context = if let Some(ref prev) = prev_task_opt {
            let mut g = prev.lock();
            &mut g.context as *mut _
        } else {
            panic!("RRScheduler: no current task to schedule from");
        };

        // 轮转策略：旧任务若仍可运行，放回运行队列尾
        if let Some(prev) = &prev_task_opt {
            let still_running = { prev.lock().state == TaskState::Running };
            if still_running {
                self.run_queue.add_task(prev.clone());
            }
        }

        // 在切换前，更新当前任务与时间片
        current_cpu().lock().current_task = Some(next_task.clone());
        self.current_slice = self.time_slice;

        Some(SwitchPlan {
            old: old_ctx_ptr,
            new: new_ctx_ptr,
        })
    }

    fn add_task(&mut self, task: SharedTask) {
        let state = { task.lock().state };
        match state {
            TaskState::Running => {
                self.run_queue.add_task(task);
            }
            _ => {
                self.wait_queue.add_task(task);
            }
        }
    }

    fn next_task(&mut self) -> Option<SharedTask> {
        self.run_queue.pop_task()
    }

    fn yield_task(&mut self) {
        schedule();
    }

    fn sleep_task(&mut self, task: SharedTask) {
        self.run_queue.remove_task(&task);

        {
            task.lock().state = TaskState::Interruptable;
        }

        if !self.wait_queue.contains(&task) {
            self.wait_queue.add_task(task.clone());
        }

        let mut cpu = current_cpu().lock();
        if let Some(cur) = &mut cpu.current_task {
            if Arc::ptr_eq(cur, &task) {
                cpu.current_task = None;
                // schedule 不会返回
                drop(cpu);
                schedule();
            }
        }
    }

    fn wake_up(&mut self, task: SharedTask) {
        self.wait_queue.remove_task(&task);

        {
            task.lock().state = TaskState::Running;
        }

        if !self.run_queue.contains(&task) {
            self.run_queue.add_task(task.clone());
        }
    }

    fn exit_task(&mut self, task: SharedTask, code: i32) {
        let state: TaskState;
        {
            let mut t = task.lock();
            state = t.state;
            t.exit_code = Some(code);
            t.state = TaskState::Stopped;
        }
        match state {
            TaskState::Running => {
                self.run_queue.remove_task(&task);
            }
            _ => {
                self.wait_queue.remove_task(&task);
            }
        }

        let mut cpu = current_cpu().lock();
        if let Some(cur) = &mut cpu.current_task {
            if Arc::ptr_eq(cur, &task) {
                cpu.current_task = None;
                // schedule 不会返回
                drop(cpu);
                schedule();
            }
        }
    }
}
