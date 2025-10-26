#![allow(dead_code)]

use core::sync::atomic::AtomicPtr;

use alloc::sync::Arc;

use crate::{
    arch::{kernel::context::Context, trap::usertrap::TrapFrame},
    kernel::task::{TID_ALLOCATOR, task_state::TaskState},
    mm::{
        address::{PageNum, UsizeConvert},
        frame_allocator::FrameTracker,
        memory_space::MemorySpace,
        physmem::physical_page_alloc,
    },
};

/// 任务
/// 存放任务的核心信息
/// OPTIMIZE: 简单起见目前的设计中，Task 结构体包含了所有信息，包括调度相关的信息和资源管理相关的信息。
///           未来可以考虑将其拆分为 TaskInfo 和 TaskStruct 两个部分，以提高访问效率和模块化程度。
/// XXX: 注意并发访问的问题，某些字段可能需要使用原子类型或锁进行保护。
/// TODO: 任务的更多字段和方法待实现，由于部分相关子系统尚未实现，暂时留空
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
    pub tid: u32,
    /// 任务的所属进程id
    /// NOTE: 由于采用了统一的任务模型，一个任务组内任务的 pid 是相同的，等于父任务的 pid 而父任务的 pid 等于自己的 tid
    pub pid: u32,
    /// 父任务的id
    pub ptid: u32,
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
    pub exit_code: Option<i32>,
}

impl Task {
    /// 创建一个新的内核任务
    /// # 参数
    /// * `ptid`: 父任务ID
    /// # 返回值
    /// 新创建的任务
    pub fn ktask_create(ptid: u32) -> Self {
        Self::new(ptid, None)
    }

    /// 创建一个新的用户任务
    /// # 参数
    /// * `ptid`: 父任务ID
    /// * `memory_space`: 任务的内存空间
    /// # 返回值
    /// 新创建的任务
    pub fn utask_create(ptid: u32, memory_space: Arc<MemorySpace>) -> Self {
        Self::new(ptid, Some(memory_space))
    }

    /// 判断该任务是否为内核线程
    pub fn is_kernel_thread(&self) -> bool {
        self.memory_space.is_none()
    }

    fn new(ptid: u32, memory_space: Option<Arc<MemorySpace>>) -> Self {
        let kstack_tracker =
            physical_page_alloc().expect("Failed to allocate kernel stack for new Task");
        let id = TID_ALLOCATOR.allocate();
        Task {
            context: Context::zero_init(),
            preempt_count: 0,
            priority: 0,
            processor_id: 0,
            state: TaskState::Running,
            tid: id,
            pid: id,
            ptid,
            // TODO: 以后改成虚拟地址
            kstack_base: kstack_tracker.ppn().end_addr().as_usize(),
            kstack_tracker,
            trap_frame_ptr: AtomicPtr::new(core::ptr::null_mut()),
            memory_space,
            exit_code: None,
        }
    }
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
