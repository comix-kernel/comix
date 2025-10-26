use alloc::sync::Arc;
use lazy_static::lazy_static;

mod task_state;
mod task_struct;
mod tid_allocator;

pub use task_state::TaskState;
pub use task_struct::Task as TaskStruct;

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
/// 包含新创建任务的 Arc<Task>
pub fn kthread_spawn(_entry_point: fn()) -> Arc<TaskStruct> {
    // 1. 分配内核栈 (假设 FrameTracker::alloc_one() 存在)
    // let kstack_tracker = FrameTracker::alloc_one().expect("Failed to allocate kernel stack");
    // let kstack_paddr = kstack_tracker.get_paddr();

    // 2. 将物理页映射到连续的虚拟地址 (kstack_base)
    // NOTE: 内核线程共享内核地址空间，映射逻辑相对简单

    // 3. 构建 Task 实例
    // let task = Task { /* ... 初始化字段 ... */ };

    // 4. 将任务加入全局任务队列
    // SCHEDULER.add_task(task.clone());

    unimplemented!("kthread_spawn 核心逻辑尚未实现")
}
