use crate::{
    kernel::{
        TaskQueue,
        scheduler::{sleep_task, wake_up},
        task::SharedTask,
    },
    sync::raw_spin_lock::RawSpinLock,
};

pub struct WaitQueue {
    // 等待队列中的任务
    tasks: TaskQueue,
    lock: RawSpinLock,
}

impl WaitQueue {
    pub fn new() -> Self {
        WaitQueue {
            tasks: TaskQueue::new(),
            lock: RawSpinLock::new(),
        }
    }

    pub fn sleep(&mut self, task: SharedTask) {
        let _g = self.lock.lock();
        self.tasks.add_task(task.clone());
        sleep_task(task, true);
    }

    pub fn wake_up(&mut self, task: &SharedTask) {
        let _g = self.lock.lock();
        if self.tasks.contains(task) {
            self.tasks.remove_task(task);
            wake_up(task.clone());
        }
    }

    pub fn wake_up_one(&mut self) {
        if let Some(task) = self.tasks.pop_task() {
            wake_up(task);
        }
    }

    pub fn wake_up_all(&mut self) {
        while let Some(task) = self.tasks.pop_task() {
            wake_up(task);
        }
    }
}

unsafe impl Send for WaitQueue {}
unsafe impl Sync for WaitQueue {}
