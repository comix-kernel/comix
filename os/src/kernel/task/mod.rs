use core::{hint, sync::atomic::Ordering};

use alloc::sync::Arc;
use lazy_static::lazy_static;

mod task_state;
mod task_struct;
mod tid_allocator;

pub use task_state::TaskState;
pub use task_struct::Task as TaskStruct;

pub type SharedTask = Arc<SpinLock<TaskStruct>>;

use crate::{
    arch::trap::{TrapFrame, restore},
    kernel::{
        cpu::current_cpu,
        scheduler::{SCHEDULER, Scheduler},
    },
    sync::spin_lock::SpinLock,
};

lazy_static! {
    static ref TID_ALLOCATOR: tid_allocator::TidAllocator = tid_allocator::TidAllocator::new();
}

/// 创建一个新的内核线程并返回其 Arc 包装
///
/// 该函数负责：
/// 1. 分配 Task 结构体本身，并用 Arc 包装
/// 2. 分配内核栈物理页帧 (FrameTracker)
/// 3. 将内核栈映射到虚拟地址空间 (VMM 逻辑)
/// 4. 初始化 Task Context，设置栈指针和入口点
/// 5. 将新的 Task 加入调度器队列
///
/// # 参数
/// * `entry_point`: 线程开始执行的函数地址
///
/// # 返回值
/// Task id
#[allow(dead_code)]
pub fn kthread_spawn(entry_point: fn()) -> u32 {
    let entry_addr = entry_point as usize;
    let ppid = {
        let cur_cpu = current_cpu().lock();
        let cur_task = cur_cpu.current_task.as_ref().unwrap();
        cur_task.lock().pid
    };
    // 分配 Task 结构体和内核栈
    let mut task = TaskStruct::ktask_create(ppid);
    task.init_kernel_thread_context(entry_addr);

    let tid = task.tid;

    // 将任务加入全局任务队列
    SCHEDULER.lock().add_task(into_shared(task));

    tid
}

/// 把已初始化的 TaskStruct 包装为共享任务句柄
pub fn into_shared(task: TaskStruct) -> SharedTask {
    Arc::new(SpinLock::new(task))
}

fn a() {
    loop {
        for _ in 0..1000 {
            core::hint::spin_loop();
        }
        print!("A");
    }
}

fn b() {
    loop {
        for _ in 0..1000 {
            core::hint::spin_loop();
        }
        print!("B");
    }
}

/// 内核的第一个任务
/// 在初始化完成后由调度器运行
/// TODO: 现在只是一个空循环
fn kinit() {
    kthread_spawn(a);
    kthread_spawn(b);
    // unsafe { intr::enable_interrupts() };
    loop {
        for _ in 0..1000 {
            core::hint::spin_loop();
        }
        print!("C");
        hint::spin_loop();
    }
}

pub fn kinit_task() -> SharedTask {
    let mut task = TaskStruct::ktask_create(0); // kinit 没有父任务
    task.init_kernel_thread_context(kinit as usize);
    into_shared(task)
}

/// 新创建的线程发生第一次调度时会从 forkret 开始执行
#[allow(dead_code)]
pub fn forkret() {
    let fp: *mut TrapFrame;
    {
        let cpu = current_cpu().lock();
        let task = cpu.current_task.as_ref().unwrap();
        fp = task.lock().trap_frame_ptr.load(Ordering::SeqCst);
    }
    unsafe { restore(&*fp) };
}
