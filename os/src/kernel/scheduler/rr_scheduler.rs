//! 轮转调度器模块
//!
//! 实现了一个简单的轮转调度器（Round-Robin Scheduler）
use crate::{
    arch::kernel::context::Context,
    kernel::{
        TaskState,
        cpu::current_cpu,
        scheduler::{Scheduler, SwitchPlan, TaskQueue},
        task::SharedTask,
    },
};

const DEFAULT_TIME_SLICE: usize = 1; // 默认时间片长度

/// 简单的轮转调度器实现
/// 每个任务按顺序轮流获得 CPU 时间片
/// 约束：
/// 1. 要求开始调度后任何时刻，至少有一个任务处于运行状态
// XXX: 现在的实现是单核的。且没有支持内核抢占。
pub struct RRScheduler {
    // 运行队列
    run_queue: TaskQueue,
    // 时间片长度（以时钟中断滴答数为单位）
    time_slice: usize,
    // 当前时间片剩余时间
    current_slice: usize,
}

impl RRScheduler {
    /// 创建一个空的调度器（const 版本）
    /// 用于静态数组初始化
    pub const fn empty() -> Self {
        RRScheduler {
            run_queue: TaskQueue::empty(),
            time_slice: DEFAULT_TIME_SLICE,
            current_slice: DEFAULT_TIME_SLICE,
        }
    }

    /// 获取调度器中的任务数量
    pub fn task_count(&self) -> usize {
        self.run_queue.len()
    }

    /// 检查调度器是否为空
    pub fn is_empty(&self) -> bool {
        self.run_queue.is_empty()
    }

    /// 更新当前时间片计数器
    /// # 返回值
    /// 如果时间片用尽，返回 true；否则返回 false
    pub fn update_time_slice(&mut self) -> bool {
        if self.current_slice > 0 {
            self.current_slice -= 1;
        }
        self.current_slice == 0
    }

    /// 重置时间片（在任务切换后调用）
    fn reset_time_slice(&mut self) {
        self.current_slice = self.time_slice;
    }
}

impl Scheduler for RRScheduler {
    fn new() -> Self {
        RRScheduler {
            run_queue: TaskQueue::new(),
            time_slice: DEFAULT_TIME_SLICE,
            current_slice: DEFAULT_TIME_SLICE,
        }
    }

    fn next_task(&mut self) -> Option<SwitchPlan> {
        let _guard = crate::sync::PreemptGuard::new();

        let cpu_id = crate::arch::kernel::cpu::cpu_id();
        crate::pr_debug!(
            "[Scheduler] CPU {} next_task called, queue size: {}",
            cpu_id,
            self.run_queue.len()
        );

        // 选择下一个可运行任务
        let next_task = match self.run_queue.pop_task() {
            Some(t) => t,
            None => {
                // 没有可运行任务：
                // - 如果当前任务仍为 Running，则继续运行它（不切换）。
                // - 否则（已阻塞/退出），切换到本 CPU 的 idle 任务。
                let prev_task = crate::kernel::current_cpu()
                    .current_task
                    .as_ref()
                    .expect("RRScheduler: no current task")
                    .clone();

                let prev_running = { prev_task.lock().state == TaskState::Running };
                if prev_running {
                    return None;
                }

                let idle = crate::kernel::current_cpu()
                    .idle_task
                    .as_ref()
                    .expect("idle_task not set")
                    .clone();

                // 切到 idle
                crate::kernel::current_cpu().switch_task(idle.clone());

                let new_ctx_ptr: *const Context = {
                    let g = idle.lock();
                    &g.context as *const _
                };
                let old_ctx_ptr: *mut Context = {
                    let mut g = prev_task.lock();
                    &mut g.context as *mut _
                };

                return Some(SwitchPlan {
                    old: old_ctx_ptr,
                    new: new_ctx_ptr,
                });
            }
        };

        // 读取当前任务，避免产生 None 窗口
        let prev_task = {
            current_cpu()
                .current_task
                .as_ref()
                .expect("RRScheduler: no current task to schedule from")
                .clone()
        };

        // 切换到新任务（也会切换地址空间）
        current_cpu().switch_task(next_task.clone());

        // 准备上下文指针
        let new_ctx_ptr: *const Context = {
            let g = next_task.lock();
            &g.context as *const _
        };
        let old_ctx_ptr: *mut Context = {
            let mut g = prev_task.lock();
            &mut g.context as *mut _
        };

        // 轮转：旧任务若仍可运行，放回运行队列尾
        {
            let still_running = { prev_task.lock().state == TaskState::Running };
            if still_running {
                self.run_queue.add_task(prev_task.clone());
            }
        }

        // 更新 on_cpu 字段和时间片
        {
            let cpu_id = crate::arch::kernel::cpu::cpu_id();
            next_task.lock().on_cpu = Some(cpu_id);
        }
        self.reset_time_slice();

        Some(SwitchPlan {
            old: old_ctx_ptr,
            new: new_ctx_ptr,
        })
    }

    fn add_task(&mut self, task: SharedTask) {
        let (state, tid) = {
            let t = task.lock();
            (t.state, t.tid)
        };
        match state {
            TaskState::Running => {
                self.run_queue.add_task(task);
                crate::pr_debug!(
                    "[Scheduler] Task {} added to run queue, new size: {}",
                    tid,
                    self.run_queue.len()
                );
            }
            _ => {
                panic!("RRScheduler: can only add running tasks to scheduler");
            }
        }
    }

    fn sleep_task(&mut self, task: SharedTask, receive_signal: bool) {
        {
            task.lock().state = if receive_signal {
                TaskState::Interruptible
            } else {
                TaskState::Uninterruptible
            };
        }

        self.run_queue.remove_task(&task);
    }

    fn wake_up(&mut self, task: SharedTask) {
        {
            task.lock().state = TaskState::Running;
        }

        if !self.run_queue.contains(&task) {
            self.run_queue.add_task(task);
        }
    }

    fn exit_task(&mut self, task: SharedTask) {
        {
            task.lock().state = TaskState::Zombie;
        }

        self.run_queue.remove_task(&task);
    }

    fn sleep_task_with_guard(
        &mut self,
        task: &mut crate::sync::SpinLockGuard<'_, crate::kernel::TaskStruct>,
        stask: SharedTask,
        receive_signal: bool,
    ) {
        task.state = if receive_signal {
            TaskState::Interruptible
        } else {
            TaskState::Uninterruptible
        };

        self.run_queue.remove_task(&stask);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        kassert,
        kernel::{cpu::current_cpu, task::TaskStruct},
        test_case,
    };

    fn mk_task(tid: u32) -> SharedTask {
        TaskStruct::new_dummy_task(tid).into_shared()
    }

    // // 基础轮转：current=T0，队列[T1,T2]，三次切换应依次运行 T1 -> T2 -> T0
    // test_case!(test_rr_prepare_switch_round_robin_order, {
    //     // 设置当前任务
    //     let t0 = mk_task(10);
    //     current_cpu().lock().current_task = Some(t0.clone());

    //     // 构造调度器并加入待运行任务
    //     let mut rr = RRScheduler::new();
    //     let t1 = mk_task(11);
    //     let t2 = mk_task(12);
    //     rr.add_task(t1.clone());
    //     rr.add_task(t2.clone());

    //     // 第一次切换：next 应为 t1，prev=t0 被放回队列
    //     let plan1 = rr.prepare_switch().expect("no switch plan 1");
    //     kassert!(plan1.old as usize != 0 && plan1.new as usize != 0);
    //     let cur1 = {
    //         let g = current_cpu().lock();
    //         g.current_task.as_ref().unwrap().lock().tid
    //     };
    //     kassert!(cur1 == 11);

    //     // 第二次切换：current=t1，next=t2，prev=t1 放回队列
    //     let plan2 = rr.prepare_switch().expect("no switch plan 2");
    //     kassert!(plan2.old as usize != 0 && plan2.new as usize != 0);
    //     let cur2 = {
    //         let g = current_cpu().lock();
    //         g.current_task.as_ref().unwrap().lock().tid
    //     };
    //     kassert!(cur2 == 12);

    //     // 第三次切换：current=t2，next 应为 t0（被回收至队列尾）
    //     let plan3 = rr.prepare_switch().expect("no switch plan 3");
    //     kassert!(plan3.old as usize != 0 && plan3.new as usize != 0);
    //     let cur3 = {
    //         let g = current_cpu().lock();
    //         g.current_task.as_ref().unwrap().lock().tid
    //     };
    //     kassert!(cur3 == 10);
    // });

    // // 添加任务：仅允许 Running 状态
    // test_case!(test_rr_add_task_only_running, {
    //     let mut rr = RRScheduler::new();
    //     let stopped = mk_task(21);
    //     {
    //         let mut g = stopped.lock();
    //         g.state = TaskState::Stopped;
    //     }
    //     let result = catch_unwind(AssertUnwindSafe(|| {
    //         rr.add_task(stopped);
    //     }));
    //     kassert!(result.is_err());
    // });

    // sleep / wake：sleep 后应不在队列且状态更新；wake 后回到队列且为 Running
    test_case!(test_rr_sleep_and_wakeup, {
        // 需要一个当前任务以便 prepare_switch 不报错；本测试不调用 prepare_switch，但保持一致设定
        {
            let _guard = crate::sync::PreemptGuard::new();
            current_cpu().current_task = Some(mk_task(30));
        }

        let mut rr = RRScheduler::new();
        let t = mk_task(31);
        rr.add_task(t.clone());

        // 休眠
        rr.sleep_task(t.clone(), false);
        {
            let g = t.lock();
            kassert!(matches!(g.state, TaskState::Uninterruptible));
        }
        kassert!(!rr.run_queue.contains(&t));

        // 唤醒
        rr.wake_up(t.clone());
        {
            let g = t.lock();
            kassert!(matches!(g.state, TaskState::Running));
        }
        kassert!(rr.run_queue.contains(&t));
    });

    // 任务退出：应设置状态为 Zombie，并从队列移除
    test_case!(test_rr_exit_task, {
        {
            let _guard = crate::sync::PreemptGuard::new();
            current_cpu().current_task = Some(mk_task(40));
        }

        let mut rr = RRScheduler::new();
        let t = mk_task(41);
        rr.add_task(t.clone());

        rr.exit_task(t.clone());
        {
            let g = t.lock();
            kassert!(matches!(g.state, TaskState::Zombie));
        }
        kassert!(!rr.run_queue.contains(&t));
    });

    // 时间片更新：手动将 current_slice 置 1，update 后应返回 true 并重置为 time_slice
    test_case!(test_rr_update_time_slice, {
        let mut rr = RRScheduler::new();
        rr.current_slice = 1; // 直接操纵以触发用尽路径
        let expired = rr.update_time_slice();
        kassert!(expired);
        kassert!(rr.current_slice == rr.time_slice);
    });
}
