use alloc::sync::Arc;

use crate::{
    arch::kernel::{context::Context, switch},
    kernel::{
        TaskState,
        cpu::current_cpu,
        scheduler::{Scheduler, task_queue::TaskQueue},
        task::SharedTask,
    },
};

const DEFAULT_TIME_SLICE: usize = 5; // 默认时间片长度（以时钟中断滴答数为单位）

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
    #[allow(dead_code)]
    pub fn update_time_slice(&mut self) {
        if self.current_slice > 0 {
            self.current_slice -= 1;
        }
        if self.current_slice == 0 {
            self.current_slice = self.time_slice;
            // XXX: 要不要分为下半部处理？
            self.schedule();
        }
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

    fn schedule(&mut self) {
        let next_task = match self.next_task() {
            Some(t) => t,
            None => {
                return;
            }
        };

        let mut next_guard = next_task.lock();
        let mut cpu = current_cpu().lock();
        let next_ctx_ptr = &mut next_guard.context as *mut _;
        let old_ctx_ptr: *mut Context = if let Some(cur_arc) = &cpu.current_task {
            let mut cur_guard = cur_arc.lock();
            &mut cur_guard.context as *mut _
        } else {
            // 约束1
            panic!("no current task to schedule from");
        };

        cpu.current_task = Some(next_task.clone());

        // 在调用 switch 前释放所有锁（否则死锁），但指针在栈帧内仍有效
        drop(cpu);
        drop(next_guard);

        unsafe {
            switch(old_ctx_ptr, next_ctx_ptr);
        }
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
        self.schedule();
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
                self.schedule();
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
                self.schedule();
            }
        }
    }
}
