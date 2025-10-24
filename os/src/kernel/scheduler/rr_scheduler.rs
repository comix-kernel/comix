use crate::{
    arch::kernel::switch,
    kernel::{
        scheduler::{RunQueue, Scheduler, WaitQueue},
        task::{Task, TaskState},
    },
};

/// 简单的轮转调度器实现
/// 每个任务按顺序轮流获得 CPU 时间片
// XXX: 现在的实现是单核的。
pub struct RRScheduler {
    // 运行队列
    run_queue: RunQueue,
    // 睡眠队列
    sleep_queues: WaitQueue,
    // 时间片长度（以时钟中断滴答数为单位）
    time_slice: usize,
    // 当前时间片剩余时间
    current_slice: usize,
    // 正在运行的任务
    current_task: Option<Task>,
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
            run_queue: RunQueue::new(),
            sleep_queues: WaitQueue::new(),
            time_slice: 5, // 假设每个任务的时间片为5个时钟中断滴答
            current_slice: 5,
            current_task: None,
        }
    }

    fn schedule(&mut self) {
        let next_task = self.next_task();

        if let Some(current_task) = self.current_task.as_mut() {
            unsafe {
                switch(
                    &mut current_task.context as *mut _,
                    &next_task.context as *const _,
                );
            }
        }

        // 将 next_task 放到 current_task
        // XXX: 注意地址稳定性问题,不要对栈上的临时 next_task 取指针然后再移动它
        self.current_task = Some(next_task);
        match self.run_queue.pop_task() {
            Some(next_task) => {
                if let Some(current_task) = self.current_task.as_mut() {
                    unsafe {
                        switch(
                            &mut current_task.context as *mut _,
                            &next_task.context as *const _,
                        );
                    }
                }
                self.current_task = Some(next_task);
                self.current_slice = self.time_slice;
            }
            None => {
                self.current_slice = self.time_slice;
            }
        }
    }

    fn add_task(&mut self, task: Task) {
        match task.state {
            TaskState::Running => {
                // 将任务添加到运行队列
                self.run_queue.queue.push(task);
            }
            _ => {
                // 将任务添加到睡眠队列
                self.sleep_queues.queue.push(task);
            }
        }
    }

    fn next_task(&mut self) -> Task {
        // 从运行队列中选择下一个任务
        if let Some(task) = self.run_queue.pop_task() {
            task
        } else {
            panic!("No runnable tasks available");
        }
    }

    fn yield_task(&mut self) {
        self.schedule();
    }

    fn sleep_task(&mut self, _wq: &WaitQueue) {
        todo!()
    }

    fn wake_up(&mut self, _wq: &WaitQueue) {
        todo!()
    }

    fn exit_task(&mut self, _code: i32) {
        todo!()
    }
}
