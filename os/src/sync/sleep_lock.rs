use crate::{kernel::WaitQueue, sync::raw_spin_lock::RawSpinLock};

pub struct SleepLock {
    locked: bool,
    guard: RawSpinLock,
    queue: WaitQueue,
}

impl SleepLock {
    pub fn new() -> Self {
        SleepLock {
            locked: false,
            guard: RawSpinLock::new(),
            queue: WaitQueue::new(),
        }
    }

    pub fn lock(&mut self) {
        loop {
            let _g = self.guard.lock();
            if !self.locked {
                self.locked = true;
                drop(_g);
                break;
            }
            drop(_g);
            crate::kernel::schedule();
        }
    }

    pub fn unlock(&mut self) {
        let _ = self.guard.lock();
        self.locked = false;
        self.queue.wake_up_one();
    }
}
