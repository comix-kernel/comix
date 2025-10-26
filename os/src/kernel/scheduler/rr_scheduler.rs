use alloc::sync::Arc;

use crate::kernel::{
    TaskState,
    scheduler::{Scheduler, task_queue::TaskQueue},
    task::SharedTask,
};

const DEFAULT_TIME_SLICE: usize = 5; // 默认时间片长度（以时钟中断滴答数为单位）

/// 简单的轮转调度器实现
/// 每个任务按顺序轮流获得 CPU 时间片
// XXX: 现在的实现是单核的。
pub struct RRScheduler {
    // 运行队列
    run_queue: TaskQueue,
    // 等待队列
    wait_queue: TaskQueue,
    // 时间片长度（以时钟中断滴答数为单位）
    time_slice: usize,
    // 当前时间片剩余时间
    current_slice: usize,
    // 正在运行的任务
    current_task: Option<SharedTask>,
}

impl RRScheduler {
    #[allow(dead_code)]
    pub fn update_time_slice(&mut self) {
        if self.current_slice > 0 {
            self.current_slice -= 1;
        }
        if self.current_slice == 0 {
            self.current_slice = self.time_slice;
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
            current_task: None,
        }
    }

    fn schedule(&mut self) {
        if self.run_queue.is_empty() {
            return;
        }

        let prev_task = self.current_task.take();

        if let Some(task) = prev_task {
            let state = unsafe { task.lock().state };

            match state {
                // 如果是 Running 状态，说明是时间片用尽，重新加入运行队列
                TaskState::Running => {
                    self.run_queue.add_task(task);
                }
                // 如果是 Sleeping/Interruptable/Stopped/Terminated 等状态，
                // 那么它已经被 sleep_task/exit_task 处理了，无需再加入 run_queue
                _ => {
                    // 如果是 sleep/exit 导致的 schedule，任务已经被移除，这里无需操作
                }
            }
        }

        let next_task = self.next_task();
        self.current_task = Some(next_task);

        // 4. 触发上下文切换 (此处省略)
    }

    fn add_task(&mut self, task: SharedTask) {
        match unsafe { task.lock().state } {
            TaskState::Running => {
                self.run_queue.add_task(task);
            }
            _ => {
                self.wait_queue.add_task(task);
            }
        }
    }

    fn next_task(&mut self) -> SharedTask {
        if let Some(task) = self.run_queue.pop_task() {
            task
        } else {
            panic!("No runnable tasks available");
        }
    }

    fn yield_task(&mut self) {
        self.schedule();
    }

    fn sleep_task(&mut self, task: SharedTask) {
        self.run_queue.remove_task(&task);

        unsafe { task.lock().state = TaskState::Interruptable };

        if !self.wait_queue.contains(&task) {
            self.wait_queue.add_task(task.clone());
        }

        if let Some(cur) = &self.current_task {
            if Arc::ptr_eq(cur, &task) {
                self.current_task = None;
                self.schedule();
            }
        }
    }

    fn wake_up(&mut self, task: SharedTask) {
        self.wait_queue.remove_task(&task);

        unsafe { task.lock().state = TaskState::Running };

        if !self.run_queue.contains(&task) {
            self.run_queue.add_task(task.clone());
        }
    }

    fn exit_task(&mut self, task: SharedTask, code: i32) {
        let state: TaskState;
        unsafe {
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
        if let Some(cur) = &self.current_task {
            if Arc::ptr_eq(cur, &task) {
                self.current_task = None;
                self.schedule();
            }
        }
    }
}
