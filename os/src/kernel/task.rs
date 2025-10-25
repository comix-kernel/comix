#![allow(dead_code)]

use core::sync::atomic::AtomicPtr;

use alloc::sync::Arc;

use crate::{
    arch::{kernel::context::Context, trap::usertrap::TrapFrame, },
    mm::{frame_allocator::FrameTracker, memory_space::MemorySpace},
};

/// 任务
/// 存放任务的核心信息
/// OPTIMIZE: 简单起见目前的设计中，Task 结构体包含了所有信息，包括调度相关的信息和资源管理相关的信息。
///           未来可以考虑将其拆分为 TaskInfo 和 TaskStruct 两个部分，以提高访问效率和模块化程度。
#[derive(Debug)]
pub struct Task {
    /// 任务的上下文信息，用于任务切换
    pub context: Context,
    /// 任务的抢占计数器，表示当前任务被禁止抢占的次数
    /// 当该值大于0时，表示任务处于不可抢占状态
    pub preempt_count: usize,
    /// 任务的优先级，数值越小优先级越高
    pub priority: u8,
    /// 任务所在的处理器id
    pub processor_id: usize,
    /// 任务当前的状态
    pub state: TaskState,
    /// 任务的id
    pub tid: usize,
    /// 任务的所属进程id
    /// NOTE: 由于采用了统一的任务模型，一个任务组内任务的 pid 是相同的，等于父任务的 pid 而父任务的 pid 等于自己的 tid
    pub pid: usize,
    /// 父任务的id
    pub ptid: usize,
    /// 内核栈基址
    pub kstack_base: usize,
    /// 内核栈跟踪器
    pub kstack_tracker: FrameTracker,
    /// 中断上下文。指向当前任务内核栈上的 TrapFrame，仅在任务被中断时有效。
    /// XXX: AtomicPtr or *mut？
    pub trap_frame_ptr: AtomicPtr<TrapFrame>,
    /// 任务的内存空间
    /// 对于内核任务，该字段为 None
    pub memory_space: Option<Arc<MemorySpace>>,
    /// 退出码
    pub exit_code: isize,
    // TODO: 由于部分相关子系统尚未实现，暂时留空
}

/// 任务状态
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TaskState {
    /// 可执行。正在执行或可被调度执行
    Running,
    /// 停止。没有也不能执行
    Stopped,
    /// 等待可中断的事件。可以被信号等中断唤醒
    Interruptable,
    /// 等待不可中断的事件。不能被信号等中断唤醒
    Uninterruptable,
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
pub fn kthread_spawn(_entry_point: fn()) -> Arc<Task> {
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

// /// 关于任务的管理信息
// /// 存放与调度器、任务状态、队列相关的、需要高频访问和修改的数据。
// /// 主要由调度器子系统使用。
// pub struct TaskInfo {}

// /// 关于任务的资源信息
// /// 存放与进程资源、内存管理、I/O 权限、用户 ID 等相关的、相对稳定或低频访问的数据。
// /// 主要由内存管理子系统和权限管理子系统使用。
// #[allow(dead_code)]
// pub struct TaskStruct {}
