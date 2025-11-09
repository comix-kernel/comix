//! 任务结构体定义
//!
//! 包含任务的核心信息，如上下文、状态、内存空间等
#![allow(dead_code)]
use core::{
    ptr,
    sync::atomic::{AtomicPtr, Ordering},
};

use alloc::sync::Arc;

use crate::{
    arch::{
        kernel::{context::Context, task::setup_stack_layout},
        trap::TrapFrame,
    },
    kernel::task::{forkret, task_state::TaskState, terminate_task},
    mm::{
        address::{ConvertablePaddr, PageNum, UsizeConvert},
        frame_allocator::{FrameRangeTracker, FrameTracker},
        memory_space::MemorySpace,
    },
    println,
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
    pub tid: u32,
    /// 任务的所属进程id
    /// NOTE: 由于采用了统一的任务模型，一个任务组内任务的 pid 是相同的，等于父任务的 pid 而父任务的 pid 等于自己的 tid
    pub pid: u32,
    /// 父任务的id
    pub ppid: u32,
    /// 内核栈基址
    pub kstack_base: usize,
    /// 中断上下文。指向当前任务内核栈上的 TrapFrame，仅在任务被中断时有效。
    pub trap_frame_ptr: AtomicPtr<TrapFrame>,
    /// 任务的内存空间
    /// 对于内核任务，该字段为 None
    pub memory_space: Option<Arc<MemorySpace>>,
    /// 退出码
    /// 存储任务退出时的状态码，通常用于表示任务的执行结果
    /// 由 exit 接口设置
    /// 对应于 waitpid 的 exit_status
    pub exit_code: Option<i32>,
    /// 返回值
    /// 存储线程函数的返回值，通常是一个指针大小的值 (usize)
    /// 对应于 pthread_join 的 void*
    pub return_value: Option<usize>,
    /// 内核栈跟踪器
    kstack_tracker: FrameRangeTracker,
    /// 任务的 TrapFrame 跟踪器
    trap_frame_tracker: FrameTracker,
}

impl Task {
    /// 为内核线程初始化任务上下文
    /// # 参数
    /// * `tid`: 任务ID
    /// * `pid`: 进程ID
    /// * `ppid`: 父任务ID
    /// * `kstack_tracker`: 内核栈的帧跟踪器
    /// * `trap_frame_tracker`: TrapFrame 的帧跟踪器
    /// * `entry`: 任务的入口地址
    /// # 返回值
    /// 新创建的任务
    pub fn ktask_create(
        tid: u32,
        pid: u32,
        ppid: u32,
        kstack_tracker: FrameRangeTracker,
        trap_frame_tracker: FrameTracker,
        entry: usize,
    ) -> Self {
        let mut task = Self::new(tid, pid, ppid, kstack_tracker, trap_frame_tracker, None);
        let tf = task.trap_frame_ptr.load(Ordering::SeqCst);
        task.context
            .set_kernel_thread_context(forkret as usize, task.kstack_base);
        // Safety: 此时 trap_frame_tracker 已经分配完毕且不可变更，所有权在 task 中，指针有效
        unsafe {
            (*tf).set_kernel_trap_frame(entry, terminate_task as usize, task.kstack_base);
        }
        task
    }

    /// 创建一个新的用户任务
    /// # 参数
    /// * `ppid`: 父任务ID
    /// * `memory_space`: 任务的内存空间
    /// # 返回值
    /// 新创建的任务
    pub fn utask_create(
        tid: u32,
        pid: u32,
        ppid: u32,
        kstack_tracker: FrameRangeTracker,
        trap_frame_tracker: FrameTracker,
        memory_space: Arc<MemorySpace>,
    ) -> Self {
        Self::new(
            tid,
            pid,
            ppid,
            kstack_tracker,
            trap_frame_tracker,
            Some(memory_space),
        )
    }

    /// 执行 execve 操作，替换当前任务的内存空间和上下文
    /// # 参数
    /// * `new_memory_space`: 新的内存空间
    /// * `entry_point`: 新程序的入口地址
    /// * `sp`: 新程序的栈指针
    /// * `argv`: 传递给新程序的参数列表
    /// * `envp`: 传递给新程序的环境变量列表
    pub fn execve(
        &mut self,
        new_memory_space: Arc<MemorySpace>,
        entry_point: usize,
        sp_high: usize,
        argv: &[&str],
        envp: &[&str],
    ) {
        // 1. 切换任务的地址空间对象
        self.memory_space = Some(new_memory_space);

        let tf_ptr = self.trap_frame_ptr.load(Ordering::SeqCst);

        // 注意：以下拷贝时对sp进行的操作均要求已经可以访问用户栈空间
        //      也就是说，new_memory_space 已经被激活（切换 satp）
        //      否则必须实现类似 copy_to_user 的函数来完成拷贝,不然会引发页错误
        // 2. 设置用户栈布局，包含命令行参数和环境变量
        let (new_sp, argc, argv_vec_ptr, envp_vec_ptr) = setup_stack_layout(sp_high, argv, envp);

        // 3. 配置 TrapFrame (新的上下文)
        // SAFETY: tfptr 指向的内存已经被分配且可写，并由 task 拥有
        unsafe {
            // 清零整个 TrapFrame，避免旧值泄漏到用户态
            core::ptr::write_bytes(tf_ptr, 0, 1);
            (*tf_ptr).set_user_trap_frame(
                entry_point,
                new_sp,
                self.kstack_base,
                argc,
                argv_vec_ptr,
                envp_vec_ptr,
            );
        }
    }

    // /// FIXME: 检查寄存器设置
    // /// 为用户进程在其内核栈上构造初始 TrapFrame，并设置 Context 指向 trampoline
    // /// # 参数
    // /// * `user_entry`: 用户态入口地址
    // /// * `trampoline`: 内核态恢复到用户态的 trampoline 函数地址
    // pub unsafe fn init_user_trapframe_and_context(&mut self, user_entry: usize, trampoline: usize) {
    //     let kstack_top = self.kstack_base;
    //     let tf_size = size_of::<TrapFrame>();
    //     let tf_ptr = (kstack_top - tf_size) as *mut TrapFrame;

    //     // 用零化的 TrapFrame 起始值，然后设置 epc（用户PC）等必要字段
    //     let mut tf: TrapFrame = unsafe { core::mem::zeroed() };
    //     tf.sepc = user_entry;
    //     // 如果需要，可在这里设置初始用户寄存器 a0/a1 等
    //     unsafe { ptr::write(tf_ptr, tf) };

    //     // 记录 TrapFrame 指针（可用 AtomicPtr，也可以省略并按约定计算）
    //     self.trap_frame_ptr.store(tf_ptr, Ordering::SeqCst);

    //     // 为调度器准备最小 Context：sp 指向栈顶，ra 指向 trampoline（trampoline 会从 tf_ptr 恢复并返回用户态）
    //     self.context.sp = kstack_top;
    //     self.context.ra = trampoline;
    // }

    /// 判断该任务是否为内核线程
    pub fn is_kernel_thread(&self) -> bool {
        self.memory_space.is_none()
    }

    /// 判断该任务是否为进程
    /// 对于进程，其 pid 等于 tid
    pub fn is_process(&self) -> bool {
        self.pid == self.tid
    }

    fn new(
        tid: u32,
        pid: u32,
        ppid: u32,
        kstack_tracker: FrameRangeTracker,
        trap_frame_tracker: FrameTracker,
        memory_space: Option<Arc<MemorySpace>>,
    ) -> Self {
        let trap_frame_ptr = trap_frame_tracker.ppn().start_addr().to_vaddr().as_usize();
        let kstack_base = kstack_tracker.end_ppn().start_addr().to_vaddr().as_usize();
        // 简单的 guard, 向TrapFrame所在页末位写入一个值，以防止越界访问
        // Safety: 该内存页已被分配且可写
        unsafe {
            let ptr = (trap_frame_tracker.ppn().end_addr().to_vaddr().as_usize()
                - size_of::<u8>()
                - 1) as *mut u8;
            ptr.write_volatile(0xFF);
        };
        Task {
            context: Context::zero_init(),
            preempt_count: 0,
            priority: 0,
            processor_id: 0,
            state: TaskState::Running,
            tid,
            // pid: tid,
            pid,
            ppid,
            kstack_base,
            kstack_tracker,
            trap_frame_tracker,
            trap_frame_ptr: AtomicPtr::new(trap_frame_ptr as *mut TrapFrame),
            memory_space,
            exit_code: None,
            return_value: None,
        }
    }

    #[cfg(test)]
    pub fn new_dummy_task(tid: u32) -> Self {
        use crate::mm::frame_allocator::{physical_page_alloc, physical_page_alloc_contiguous};

        let kstack_tracker =
            physical_page_alloc_contiguous(1).expect("new_dummy_task: failed to alloc kstack");
        let trap_frame_tracker =
            physical_page_alloc().expect("new_dummy_task: failed to alloc trap_frame");
        Self::new(tid, tid, 0, kstack_tracker, trap_frame_tracker, None)
    }
}

impl Drop for Task {
    fn drop(&mut self) {
        println!("Dropping Task {}", self.tid);
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
    use crate::{
        kassert,
        mm::frame_allocator::{physical_page_alloc, physical_page_alloc_contiguous},
        test_case,
    };
    use core::mem::size_of;

    // 创建内核任务的基本属性检查
    test_case!(test_ktask_create, {
        let kstack_tracker =
            physical_page_alloc_contiguous(4).expect("kthread_spawn: failed to alloc kstack");
        let trap_frame_tracker =
            physical_page_alloc().expect("kthread_spawn: failed to alloc trap_frame");
        let t = Task::ktask_create(1, 1, 0, kstack_tracker, trap_frame_tracker, 0x1000);
        kassert!(t.tid == 1);
        kassert!(t.pid == t.tid);
        kassert!(t.is_kernel_thread());
        kassert!(t.is_process());
        kassert!(t.kstack_base != 0);
        kassert!(t.trap_frame_ptr.load(Ordering::SeqCst) as usize != 0);
    });

    // new_dummy_task：应为内核线程，pid=tid，初始状态为 Running
    test_case!(test_dummy_task_basic, {
        let t = Task::new_dummy_task(7);
        kassert!(t.tid == 7);
        kassert!(t.pid == 7);
        kassert!(t.is_kernel_thread());
        kassert!(t.is_process());
        kassert!(matches!(t.state, TaskState::Running));
    });

    // is_process 与 is_kernel_thread 区分：人为创建一个“线程” pid!=tid
    test_case!(test_is_process_vs_thread, {
        let kstack_tracker = physical_page_alloc_contiguous(2).expect("alloc kstack");
        let trap_frame_tracker = physical_page_alloc().expect("alloc trap_frame");
        // 传入 pid 与 tid 不同模拟同进程内的线程
        let t = Task::ktask_create(10, 5, 5, kstack_tracker, trap_frame_tracker, 0x2000);
        kassert!(t.tid == 10);
        kassert!(t.pid == 5);
        kassert!(!t.is_process());
        kassert!(t.is_kernel_thread()); // 仍是内核线程（没有用户地址空间）
    });

    // // init_user_trapframe_and_context：验证重新定位 trap_frame 指针与入口设置
    // test_case!(test_init_user_trapframe_and_context, {
    //     let mut t = Task::new_dummy_task(3);
    //     let original_tf_ptr = t.trap_frame_ptr.load(Ordering::SeqCst) as usize;
    //     let user_entry = 0x5555_8888usize;
    //     let trampoline = 0xFFFF_FFC0_8020_9000usize;
    //     unsafe {
    //         t.init_user_trapframe_and_context(user_entry, trampoline);
    //     }
    //     let new_tf_ptr = t.trap_frame_ptr.load(Ordering::SeqCst) as usize;
    //     // 新 trap_frame 应位于内核栈顶下方 size_of::<TrapFrame>()
    //     let expect_ptr = t.kstack_base - size_of::<TrapFrame>();
    //     kassert!(new_tf_ptr == expect_ptr);
    //     kassert!(new_tf_ptr != original_tf_ptr);
    //     // 校验写入的 sepc
    //     let tf = unsafe { &*t.trap_frame_ptr.load(Ordering::SeqCst) };
    //     kassert!(tf.sepc == user_entry);
    //     // Context 设置
    //     kassert!(t.context.sp == t.kstack_base);
    //     kassert!(t.context.ra == trampoline);
    // });

    // // execve 前后的 TrapFrame 基本字段（不访问用户空间，只验证写入逻辑）
    // test_case!(test_execve_basic_trapframe_setup, {
    //     // 构造内核任务再模拟成为用户任务：直接插入一个空的 MemorySpace（若出现 API 变化需调整）
    //     // 使用 zeroed MemorySpace 仅用于测试 trap_frame 字段写入，不触发实际页表操作
    //     let mut t = Task::new_dummy_task(11);
    //     // 伪造用户地址空间（测试目的：Some 即视为用户进程）
    //     // SAFETY: 仅在测试中使用，MemorySpace 零值不会被真正激活
    //     let dummy_space: Arc<MemorySpace> = unsafe { core::mem::zeroed() };
    //     t.memory_space = Some(dummy_space);

    //     let tf_ptr = t.trap_frame_ptr.load(Ordering::SeqCst);
    //     let entry = 0x1234_5678usize;
    //     let user_sp_high = t.kstack_base & !0xFF; // 构造一个“高地址”作为栈顶
    //     let argv = ["prog", "arg1"];
    //     let envp = ["KEY=VALUE"];
    //     // 调用 execve（由于 dummy_space 不会映射，不能访问用户页，仅验证不崩溃及字段设置）
    //     // 为避免实际用户页访问，这里将字符串数组长度设小，且不真正触发用户空间写（dummy 空页表会使 SUM 写失败时 panic，若失败则跳过此测试）
    //     // 如果出现页错误，可根据真实 MemorySpace API 替换为可映射测试空间。
    //     t.execve(unsafe { core::mem::zeroed() }, entry, user_sp_high, &argv, &envp);
    //     let tf_after = unsafe { &*tf_ptr };
    //     kassert!(tf_after.sepc == entry);
    //     kassert!(tf_after.x10_a0 == argv.len());
    //     kassert!(tf_after.x1_ra == 0);
    // });
}
