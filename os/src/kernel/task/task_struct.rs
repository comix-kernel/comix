#![allow(dead_code)]

use core::{
    ptr,
    sync::atomic::{AtomicPtr, Ordering},
};

use alloc::sync::Arc;
use riscv::register::sstatus;

use crate::{
    arch::{kernel::context::Context, trap::TrapFrame},
    kernel::task::{TID_ALLOCATOR, forkret, task_state::TaskState},
    mm::{
        address::{ConvertablePaddr, PageNum, UsizeConvert},
        frame_allocator::{
            FrameRangeTracker, FrameTracker, physical_page_alloc, physical_page_alloc_contiguous,
        },
        memory_space::memory_space::MemorySpace,
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
    pub ppid: u32,
    /// 内核栈基址
    pub kstack_base: usize,
    /// 内核栈跟踪器
    pub kstack_tracker: FrameRangeTracker,
    /// 任务的 TrapFrame 跟踪器
    pub trap_frame_tracker: FrameTracker,
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
    /// * `ppid`: 父任务ID
    /// # 返回值
    /// 新创建的任务
    pub fn ktask_create(ppid: u32) -> Self {
        Self::new(ppid, None)
    }

    /// 创建一个新的用户任务
    /// # 参数
    /// * `ppid`: 父任务ID
    /// * `memory_space`: 任务的内存空间
    /// # 返回值
    /// 新创建的任务
    pub fn utask_create(ppid: u32, memory_space: Arc<MemorySpace>) -> Self {
        Self::new(ppid, Some(memory_space))
    }

    /// 为内核线程准备最小 Context：sp 指向栈顶，ra 指向线程入口
    /// # 参数
    /// * `entry`: 线程入口地址
    pub fn init_kernel_thread_context(&mut self, entry: usize) {
        let tf = self.trap_frame_ptr.load(Ordering::SeqCst);
        let mut sstatus = sstatus::read();
        sstatus.set_sie(false);
        sstatus.set_spie(true);
        sstatus.set_spp(sstatus::SPP::Supervisor);
        self.context.sp = self.kstack_base;
        self.context.ra = forkret as usize;
        unsafe {
            (*tf).x4_tp = self.pid as usize;
            (*tf).sepc = entry;
            (*tf).sstatus = sstatus.bits();
            // 内核线程的栈指针初始化为内核栈顶
            (*tf).x2_sp = self.kstack_base;
            (*tf).kernel_sp = self.kstack_base;
        }
        println!(
            "Task {}: init_kernel_thread_context: stack_base={:#x}, 
            stack.end={:#x}, tf={:#?}",
            self.tid,
            self.kstack_base,
            self.context.sp,
            unsafe { *tf }
        );
    }

    /// FIXME: 检查寄存器设置
    /// 为用户进程在其内核栈上构造初始 TrapFrame，并设置 Context 指向 trampoline
    /// # 参数
    /// * `user_entry`: 用户态入口地址
    /// * `trampoline`: 内核态恢复到用户态的 trampoline 函数地址
    pub unsafe fn init_user_trapframe_and_context(&mut self, user_entry: usize, trampoline: usize) {
        let kstack_top = self.kstack_base;
        let tf_size = size_of::<TrapFrame>();
        let tf_ptr = (kstack_top - tf_size) as *mut TrapFrame;

        // 用零化的 TrapFrame 起始值，然后设置 epc（用户PC）等必要字段
        let mut tf: TrapFrame = unsafe { core::mem::zeroed() };
        tf.sepc = user_entry;
        // 如果需要，可在这里设置初始用户寄存器 a0/a1 等
        unsafe { ptr::write(tf_ptr, tf) };

        // 记录 TrapFrame 指针（可用 AtomicPtr，也可以省略并按约定计算）
        self.trap_frame_ptr.store(tf_ptr, Ordering::SeqCst);

        // 为调度器准备最小 Context：sp 指向栈顶，ra 指向 trampoline（trampoline 会从 tf_ptr 恢复并返回用户态）
        self.context.sp = kstack_top;
        self.context.ra = trampoline;
    }

    /// 判断该任务是否为内核线程
    pub fn is_kernel_thread(&self) -> bool {
        self.memory_space.is_none()
    }

    fn new(ppid: u32, memory_space: Option<Arc<MemorySpace>>) -> Self {
        let kstack_tracker = physical_page_alloc_contiguous(4)
            .expect("Failed to allocate kernel stack for new Task");
        // OPTIMIZE: 可以节省内存使用
        let trap_frame_tracker =
            physical_page_alloc().expect("Failed to allocate frame for new Task");
        let trap_frame_ptr = trap_frame_tracker.ppn().start_addr().to_vaddr().as_usize();
        let kstack_base = kstack_tracker.end_ppn().end_addr().to_vaddr().as_usize();
        let id = TID_ALLOCATOR.allocate();
        let context = Context::zero_init();
        // 简单的 guard, 向TrapFrame所在页末位写入一个值，以防止越界访问
        unsafe {
            let ptr = (trap_frame_tracker.ppn().end_addr().to_vaddr().as_usize() - size_of::<u8>())
                as *mut u8;
            ptr.write_volatile(0xFF);
        };
        Task {
            context,
            preempt_count: 0,
            priority: 0,
            processor_id: 0,
            state: TaskState::Running,
            tid: id,
            pid: id,
            ppid,
            kstack_base,
            kstack_tracker,
            trap_frame_tracker,
            trap_frame_ptr: AtomicPtr::new(trap_frame_ptr as *mut TrapFrame),
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{kassert, test_case};

    // 创建内核任务的基本属性检查
    test_case!(test_ktask_create_basic, {
        println!("Testing: test_ktask_create_basic");
        let t = Task::ktask_create(0);
        // tid/ pid 应有效且相等
        kassert!(t.tid != 0);
        kassert!(t.pid == t.tid);
        // 默认创建为内核线程（memory_space == None）
        kassert!(t.is_kernel_thread());
        // 内核栈基址应非零
        kassert!(t.kstack_base != 0);
    });

    // 初始化内核线程上下文（sp/ra）检查
    test_case!(test_init_kernel_thread_context, {
        println!("Testing: test_init_kernel_thread_context");
        let mut t = Task::ktask_create(0);
        let entry = 0x1000usize;
        t.init_kernel_thread_context(entry);
        // sp 应为栈顶（kstack_base），ra 应为统一入口地址
        kassert!(t.context.sp == t.kstack_base);
        kassert!(t.context.ra == forkret as usize);
        // sepc 应为 entry
        kassert!(unsafe { (*t.trap_frame_ptr.load(Ordering::SeqCst)).sepc } == entry);
        // 内核线程不应在创建时拥有有效的 trap_frame_ptr
        // kassert!(t.trap_frame_ptr.load(Ordering::SeqCst).is_null());
    });
}
