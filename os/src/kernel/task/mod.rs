use alloc::sync::Arc;
use lazy_static::lazy_static;

mod task_state;
mod task_struct;
mod tid_allocator;

pub use task_state::TaskState;
pub use task_struct::Task as TaskStruct;

pub type SharedTask = Arc<SpinLock<TaskStruct>>;

use crate::{
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
pub fn kthread_spawn(entry_point: fn()) -> u32 {
    let entry_addr = entry_point as usize;
    let cur_task = current_cpu().current_task.as_ref().unwrap();
    let ppid = cur_task.pid;
    // 分配 Task 结构体和内核栈
    let mut task = TaskStruct::ktask_create(ppid);
    task.init_kernel_thread_context(entry_addr);

    let tid = task.tid;
    // TODO: 将物理页映射到连续的虚拟地址 (kstack_base)
    // NOTE: 内核线程共享内核地址空间，映射逻辑相对简单

    // 将任务加入全局任务队列
    unsafe { SCHEDULER.lock().add_task(into_shared(task)) };

    tid
}

/// 把已初始化的 TaskStruct 包装为共享任务句柄
pub fn into_shared(task: TaskStruct) -> SharedTask {
    Arc::new(SpinLock::new(task))
}
