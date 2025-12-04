//! 任务结构体定义
//!
//! 包含任务的核心信息，如上下文、状态、内存空间等
#![allow(dead_code)]
use core::sync::atomic::{AtomicPtr, Ordering};

use alloc::{sync::Arc, vec::Vec};

use crate::{
    arch::{
        kernel::{context::Context, task::setup_stack_layout},
        trap::TrapFrame,
    },
    ipc::{SignalHandlerTable, SignalPending},
    kernel::{
        WaitQueue,
        task::{forkret, task_state::TaskState},
    },
    mm::{
        address::{ConvertablePaddr, PageNum, UsizeConvert},
        frame_allocator::{FrameRangeTracker, FrameTracker},
        memory_space::MemorySpace,
    },
    pr_debug,
    sync::SpinLock,
    uapi::{
        resource::RlimitStruct,
        signal::{SignalFlags, SignalStack},
        uts_namespace::UtsNamespace,
    },
    vfs::{Dentry, FDTable},
};

/// 共享任务句柄
/// 用于在多个地方引用同一个任务实例
pub type SharedTask = Arc<SpinLock<Task>>;

/// 任务
/// 存放任务的核心信息
/// 其中的信息可以分为几大类：
/// 1. 调度运行相关的信息，如上下文、状态、优先级等
/// 2. 任务标识信息，如 tid、pid、ppid、子任务等
/// 3. 任务资源信息，如内核栈、TrapFrame、内存空间等
/// 表示进程的任务与表示线程的任务由本结构体统一表征
/// 其区别仅在于：
/// 1. 进程的 pid 等于 tid，线程的 pid 不等于 tid
/// 2. 进程的返回值通过 exit_code 字段传递，线程的返回值通过 return_value 字段传递
/// 3. 对于所有3.类信息，均通过引用计数共享。创建时任务，进程需传入新的`Arc<T>`，而线程则共享父任务的资源。
/// 注意：线程拥有自己独立的运行栈，和一套寄存器上下文。
///      TrapFrame，Context结构可以保证所有线程切换时保存和恢复寄存器状态。
///      每个任务的内核栈独立分配，互不干扰。内核线程只使用内核栈。
///      但是上层必须自己保证创建的用户线程在用户态运行时拥有独立的用户栈空间。
/// OPTIMIZE: 简单起见目前的设计中，Task 结构体包含了所有信息，包括调度相关的信息和资源管理相关的信息。
///           未来可以考虑将其拆分为 TaskInfo 和 TaskStruct 两个部分，以提高访问效率和模块化程度。
#[derive(Debug)]
pub struct Task {
    /// 任务的上下文信息，用于任务切换
    pub context: Context,
    /// 任务的抢占计数器，表示当前任务被禁止抢占的次数
    /// 当该值大于0时，表示任务处于不可抢占状态。暂未使用
    pub preempt_count: usize,
    /// 任务的优先级，数值越小优先级越高。暂未使用
    pub priority: u8,
    /// 任务所在的处理器id。暂未使用
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
    /// 任务的进程组id
    pub pgid: u32,
    /// 任务的子任务列表
    pub children: Arc<SpinLock<Vec<SharedTask>>>,
    /// 任务的等待队列
    pub wait_child: Arc<SpinLock<WaitQueue>>,
    /// 内核栈基址
    pub kstack_base: usize,
    /// 中断上下文。指向当前任务内核栈上的 TrapFrame，仅在任务被中断时有效。
    pub trap_frame_ptr: AtomicPtr<TrapFrame>,
    /// 任务的内存空间
    /// 对于内核任务，该字段为 None
    pub memory_space: Option<Arc<SpinLock<MemorySpace>>>,
    /// 退出码
    /// 存储任务退出时的状态码，通常用于表示任务的执行结果
    /// 由 exit 接口设置
    /// 对应于 waitpid 的 exit_status
    pub exit_code: Option<i32>,
    /// 内核栈跟踪器
    kstack_tracker: FrameRangeTracker,
    /// 任务的 TrapFrame 跟踪器
    trap_frame_tracker: FrameTracker,
    /// 信号屏蔽字
    pub blocked: SignalFlags,
    /// 私有待处理信号集合
    pub pending: SignalPending,
    /// 待处理信号队列
    pub shared_pending: Arc<SpinLock<SignalPending>>,
    /// 信号处理动作表
    pub signal_handlers: Arc<SpinLock<SignalHandlerTable>>,
    /// 备用信号栈信息
    pub signal_stack: Arc<SpinLock<SignalStack>>,
    /// 退出信号, 当任务退出时发送给父任务的信号
    pub exit_signal: u8,
    /// UTS 命名空间
    pub uts_namespace: Arc<SpinLock<UtsNamespace>>,
    /// 资源限制结构体
    pub rlimit: Arc<SpinLock<RlimitStruct>>,
    /// 健壮列表头地址及其大小
    pub robust_list: Option<usize>,
    /// 线程ID地址
    pub set_child_tid: usize,
    /// 线程退出时清除的线程ID地址
    pub clear_child_tid: usize,

    // === 权限和凭证 ===
    /// 任务凭证（用户、组、能力）
    pub credential: super::Credential,
    /// 文件创建掩码
    pub umask: u32,

    // === 文件系统 ===
    /// 文件描述符表
    pub fd_table: Arc<FDTable>,
    /// 文件系统信息
    pub fs: Arc<SpinLock<FsStruct>>,
}

/// 文件系统信息相关结构体
#[derive(Debug, Clone)]
pub struct FsStruct {
    /// 当前工作目录
    pub cwd: Option<Arc<Dentry>>,
    /// 根目录
    pub root: Option<Arc<Dentry>>,
}

impl FsStruct {
    pub fn new(cwd: Option<Arc<Dentry>>, root: Option<Arc<Dentry>>) -> Self {
        Self { cwd, root }
    }
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
    /// 注意：调用者必须自己初始化TrapFrame内容
    pub fn ktask_create(
        tid: u32,
        pid: u32,
        ppid: u32,
        children: Arc<SpinLock<Vec<Arc<SpinLock<Task>>>>>,
        kstack_tracker: FrameRangeTracker,
        trap_frame_tracker: FrameTracker,
        signal_handlers: Arc<SpinLock<SignalHandlerTable>>,
        blocked: SignalFlags,
        signal: Arc<SpinLock<SignalPending>>,
        uts_namespace: Arc<SpinLock<UtsNamespace>>,
        rlimit: Arc<SpinLock<RlimitStruct>>,
        fd_table: Arc<FDTable>,
        fs: Arc<SpinLock<FsStruct>>,
    ) -> Self {
        let mut task = Self::new(
            tid,
            pid,
            ppid,
            tid, // 内核线程不属于常规意义的进程组
            children,
            kstack_tracker,
            trap_frame_tracker,
            None,
            signal_handlers,
            blocked,
            signal,
            Arc::new(SpinLock::new(SignalStack::default())), // 内核线程通常不使用备用信号栈
            0,                                               // 内核线程退出不通过IPC发送信号
            uts_namespace,
            rlimit,
            fd_table,
            fs,
        );
        task.context
            .set_init_context(forkret as usize, task.kstack_base);
        task
    }

    /// 创建一个新的用户任务
    /// # 参数
    /// * `ppid`: 父任务ID
    /// * `memory_space`: 任务的内存空间
    /// # 返回值
    /// 新创建的任务
    /// 注意：调用者必须自己初始化TrapFrame内容
    pub fn utask_create(
        tid: u32,
        pid: u32,
        ppid: u32,
        pgid: u32,
        children: Arc<SpinLock<Vec<Arc<SpinLock<Task>>>>>,
        kstack_tracker: FrameRangeTracker,
        trap_frame_tracker: FrameTracker,
        memory_space: Arc<SpinLock<MemorySpace>>,
        signal_handlers: Arc<SpinLock<SignalHandlerTable>>,
        blocked: SignalFlags,
        signal: Arc<SpinLock<SignalPending>>,
        signal_stack: Arc<SpinLock<SignalStack>>,
        exit_signal: u8,
        uts_namespace: Arc<SpinLock<UtsNamespace>>,
        rlimit: Arc<SpinLock<RlimitStruct>>,
        fd_table: Arc<FDTable>,
        fs: Arc<SpinLock<FsStruct>>,
    ) -> Self {
        let mut task = Self::new(
            tid,
            pid,
            ppid,
            pgid,
            children,
            kstack_tracker,
            trap_frame_tracker,
            Some(memory_space),
            signal_handlers,
            blocked,
            signal,
            signal_stack,
            exit_signal,
            uts_namespace,
            rlimit,
            fd_table,
            fs,
        );
        task.context
            .set_init_context(forkret as usize, task.kstack_base);
        task
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
        new_memory_space: Arc<SpinLock<MemorySpace>>,
        entry_point: usize,
        sp_high: usize,
        argv: &[&str],
        envp: &[&str],
        phdr_addr: usize,
        phnum: usize,
        phent: usize,
    ) {
        // 1. 切换任务的地址空间对象
        self.memory_space = Some(new_memory_space);

        // 2. 处理文件描述符：取消共享并关闭 CLOEXEC 文件
        // execve 应该让当前进程拥有独立的 FD 表（如果之前是共享的）
        // 并且关闭所有标记为 FD_CLOEXEC 的文件
        let new_fd_table = self.fd_table.clone_table();
        new_fd_table.close_exec();
        self.fd_table = Arc::new(new_fd_table);

        let tf_ptr = self.trap_frame_ptr.load(Ordering::SeqCst);

        // 注意：以下拷贝时对sp进行的操作均要求已经可以访问用户栈空间
        //      也就是说，new_memory_space 已经被激活（切换 satp）
        //      否则必须实现类似 copy_to_user 的函数来完成拷贝,不然会引发页错误
        // 3. 设置用户栈布局，包含命令行参数和环境变量
        let (new_sp, argc, argv_vec_ptr, envp_vec_ptr) =
            setup_stack_layout(sp_high, argv, envp, phdr_addr, phnum, phent, entry_point);

        // 4. 配置 TrapFrame (新的上下文)
        // SAFETY: tfptr 指向的内存已经被分配且可写，并由 task 拥有
        unsafe {
            // 清零整个 TrapFrame，避免旧值泄漏到用户态
            core::ptr::write_bytes(tf_ptr, 0, 1);
            (*tf_ptr).set_exec_trap_frame(
                entry_point,
                new_sp,
                self.kstack_base,
                argc,
                argv_vec_ptr,
                envp_vec_ptr,
            );
        }
    }

    /// 检查是否有满足条件的子任务
    /// # 参数
    /// * `cond`: 用于检查子任务的条件闭包
    /// * `remove`: 是否在找到后从子任务列表中移除该僵尸子任务(如果是)
    /// # 返回值
    /// 如果有，返回该子任务的共享句柄. 如果没有，返回 None
    /// 注意：此函数不阻塞，调用者需持有锁
    pub fn check_child(
        &mut self,
        cond: impl FnMut(&SharedTask) -> bool,
        remove: bool,
    ) -> Option<SharedTask> {
        let mut children_guard = self.children.lock();
        if let Some(idx) = children_guard.iter().position(cond) {
            let child = children_guard[idx].clone();
            if remove && child.lock().state == TaskState::Zombie {
                children_guard.remove(idx);
            }
            return Some(child);
        }
        None
    }

    pub fn notify_child_exit(&mut self) {
        self.wait_child.lock().wake_up_one();
    }

    /// 判断该任务是否为内核线程
    pub fn is_kernel_thread(&self) -> bool {
        self.memory_space.is_none()
    }

    /// 判断该任务是否为进程 / 主线程
    /// 对于进程，其 pid 等于 tid
    pub fn is_process(&self) -> bool {
        self.pid == self.tid
    }

    /// 把已初始化的 TaskStruct 包装为共享任务句柄
    /// 返回值: 包装后的 SharedTask
    pub fn into_shared(self) -> SharedTask {
        Arc::new(SpinLock::new(self))
    }

    /// 返回一个空的子任务列表
    /// 用于创建新任务时初始化 children 字段
    pub fn empty_children() -> Arc<SpinLock<Vec<SharedTask>>> {
        Arc::new(SpinLock::new(Vec::new()))
    }

    fn new(
        tid: u32,
        pid: u32,
        ppid: u32,
        pgid: u32,
        children: Arc<SpinLock<Vec<SharedTask>>>,
        kstack_tracker: FrameRangeTracker,
        trap_frame_tracker: FrameTracker,
        memory_space: Option<Arc<SpinLock<MemorySpace>>>,
        signal_handlers: Arc<SpinLock<SignalHandlerTable>>,
        blocked: SignalFlags,
        shared_pending: Arc<SpinLock<SignalPending>>,
        signal_stack: Arc<SpinLock<SignalStack>>,
        exit_signal: u8,
        uts_namespace: Arc<SpinLock<UtsNamespace>>,
        rlimit: Arc<SpinLock<RlimitStruct>>,
        fd_table: Arc<FDTable>,
        fs: Arc<SpinLock<FsStruct>>,
    ) -> Self {
        let trap_frame_ptr = trap_frame_tracker.ppn().start_addr().to_vaddr().as_usize();
        let kstack_base = kstack_tracker.end_ppn().start_addr().to_vaddr().as_usize();

        Task {
            context: Context::zero_init(),
            preempt_count: 0,
            priority: 0,
            processor_id: 0,
            state: TaskState::Running,
            tid,
            pid,
            ppid,
            pgid,
            children,
            wait_child: Arc::new(SpinLock::new(WaitQueue::new())),
            kstack_base,
            kstack_tracker,
            trap_frame_tracker,
            trap_frame_ptr: AtomicPtr::new(trap_frame_ptr as *mut TrapFrame),
            memory_space,
            exit_code: None,
            signal_handlers,
            signal_stack,
            exit_signal,
            uts_namespace,
            rlimit,
            blocked,
            pending: SignalPending::empty(),
            shared_pending,
            robust_list: None,
            set_child_tid: 0,
            clear_child_tid: 0,
            credential: super::Credential::root(),
            umask: 0o022,
            fd_table,
            fs,
        }
    }

    #[cfg(test)]
    pub fn new_dummy_task(tid: u32) -> Self {
        use crate::{
            mm::frame_allocator::{alloc_contig_frames, alloc_frame},
            uapi::resource::INIT_RLIMITS,
        };
        let kstack_tracker =
            alloc_contig_frames(1).expect("new_dummy_task: failed to alloc kstack");
        let trap_frame_tracker = alloc_frame().expect("new_dummy_task: failed to alloc trap_frame");
        Self::new(
            tid,
            tid,
            0,
            0,
            Task::empty_children(),
            kstack_tracker,
            trap_frame_tracker,
            None,
            Arc::new(SpinLock::new(SignalHandlerTable::new())),
            SignalFlags::empty(),
            Arc::new(SpinLock::new(SignalPending::empty())),
            Arc::new(SpinLock::new(SignalStack::default())),
            0,
            Arc::new(SpinLock::new(UtsNamespace::default())),
            Arc::new(SpinLock::new(RlimitStruct::new(INIT_RLIMITS))),
            Arc::new(FDTable::new()),
            Arc::new(SpinLock::new(FsStruct::new(None, None))),
        )
    }
}

impl Drop for Task {
    fn drop(&mut self) {
        pr_debug!("Dropping Task {}", self.tid);
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

    // // 创建内核任务的基本属性检查
    // test_case!(test_ktask_create, {
    //     let kstack_tracker = alloc_contig_frames(4).expect("kthread_spawn: failed to alloc kstack");
    //     let trap_frame_tracker = alloc_frame().expect("kthread_spawn: failed to alloc trap_frame");
    //     let t = Task::ktask_create(
    //         1,
    //         1,
    //         0,
    //         Task::empty_children(),
    //         kstack_tracker,
    //         trap_frame_tracker,
    //         Arc::new(SpinLock::new(SignalHandlerTable::new())),
    //         SignalFlags::empty(),
    //         Arc::new(SpinLock::new(UtsNamespace::default())),
    //         Arc::new(SpinLock::new(RlimitStruct::new(INIT_RLIMITS))),
    //         Arc::new(FDTable::new()),
    //     );
    //     kassert!(t.tid == 1);
    //     kassert!(t.pid == t.tid);
    //     kassert!(t.is_kernel_thread());
    //     kassert!(t.is_process());
    //     kassert!(t.kstack_base != 0);
    //     kassert!(t.trap_frame_ptr.load(Ordering::SeqCst) as usize != 0);
    // });

    // new_dummy_task：应为内核线程，pid=tid，初始状态为 Running
    test_case!(test_dummy_task_basic, {
        let t = Task::new_dummy_task(7);
        kassert!(t.tid == 7);
        kassert!(t.pid == 7);
        kassert!(t.is_kernel_thread());
        kassert!(t.is_process());
        kassert!(matches!(t.state, TaskState::Running));
    });

    // // is_process 与 is_kernel_thread 区分：人为创建一个“线程” pid!=tid
    // test_case!(test_is_process_vs_thread, {
    //     let kstack_tracker = alloc_contig_frames(2).expect("alloc kstack");
    //     let trap_frame_tracker = alloc_frame().expect("alloc trap_frame");
    //     // 传入 pid 与 tid 不同模拟同进程内的线程
    //     let t = Task::ktask_create(
    //         10,
    //         5,
    //         5,
    //         Task::empty_children(),
    //         kstack_tracker,
    //         trap_frame_tracker,
    //         Arc::new(SpinLock::new(SignalHandlerTable::new())),
    //         SignalFlags::empty(),
    //         Arc::new(SpinLock::new(UtsNamespace::default())),
    //         Arc::new(SpinLock::new(RlimitStruct::new(INIT_RLIMITS))),
    //     );
    //     kassert!(t.tid == 10);
    //     kassert!(t.pid == 5);
    //     kassert!(!t.is_process());
    //     kassert!(t.is_kernel_thread()); // 仍是内核线程（没有用户地址空间）
    // });

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
