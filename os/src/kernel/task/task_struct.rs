//! 任务结构体定义
//! 包含任务的核心信息，如上下文、状态、内存空间等
#![allow(dead_code)]
use core::{
    ptr,
    sync::atomic::{AtomicPtr, Ordering},
};

use alloc::{sync::Arc, vec::Vec};
use riscv::register::sstatus;

use crate::{
    arch::{constant::STACK_ALIGN_MASK, kernel::context::Context, trap::TrapFrame},
    kernel::task::{forkret, task_state::TaskState, terminate_task},
    mm::{
        address::{ConvertablePaddr, PageNum, UsizeConvert},
        frame_allocator::{FrameRangeTracker, FrameTracker},
        memory_space::MemorySpace,
    },
};

/// 任务
/// 存放任务的核心信息
/// OPTIMIZE: 简单起见目前的设计中，Task 结构体包含了所有信息，包括调度相关的信息和资源管理相关的信息。
///           未来可以考虑将其拆分为 TaskInfo 和 TaskStruct 两个部分，以提高访问效率和模块化程度。
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
        let mut sstatus = sstatus::read();
        sstatus.set_sie(false);
        sstatus.set_spie(true);
        sstatus.set_spp(sstatus::SPP::Supervisor);
        task.context.sp = task.kstack_base;
        task.context.ra = forkret as usize;
        unsafe {
            // 暂时用tp寄存器保存pid
            (*tf).x4_tp = task.pid as usize;
            (*tf).sepc = entry;
            (*tf).sstatus = sstatus.bits();
            // 内核线程的栈指针初始化为内核栈顶
            (*tf).x2_sp = task.kstack_base;
            (*tf).kernel_sp = task.kstack_base;
            (*tf).x1_ra = terminate_task as usize;
        }
        task
        // debug 输出任务创建信息
        // println!(
        //     "Task {}: init_kernel_thread_context: stack_base={:#x},
        //     stack.end={:#x}, tf={:#?}",
        //     self.tid,
        //     self.kstack_base,
        //     self.context.sp,
        //     unsafe { *tf }
        // );
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
    // （高地址 - 栈底）
    // +-----------------------+
    // | ...                   |
    // +-----------------------+
    // | "USER=john"           | <-- envp[2] 指向这里
    // +-----------------------+
    // | "HOME=/home/john"     | <-- envp[1] 指向这里
    // +-----------------------+
    // | "SHELL=/bin/bash"     | <-- envp[0] 指向这里
    // +-----------------------+
    // | "hello world"         | <-- argv[3] 指向这里
    // +-----------------------+
    // | "arg2"                | <-- argv[2] 指向这里
    // +-----------------------+
    // | "arg1"                | <-- argv[1] 指向这里
    // +-----------------------+
    // | "./stack_layout"      | <-- argv[0] 指向这里
    // +-----------------------+     <--- 字符串存储区域开始
    // | ...                   |
    // +-----------------------+     <--- 进入 main 时的栈指针 (sp) 附近
    // | char* envp[0] (NULL)  |
    // +-----------------------+
    // | char* envp[2]         | --> 指向上面的 "USER=john"
    // | char* envp[1]         | --> 指向上面的 "HOME=/home/john"
    // | char* envp[0]         | --> 指向上面的 "SHELL=/bin/bash"
    // +-----------------------+
    // | char* argv[argc] (NULL)|
    // +-----------------------+
    // | char* argv[3]         | --> 指向上面的 "hello world"
    // | char* argv[2]         | --> 指向上面的 "arg2"
    // | char* argv[1]         | --> 指向上面的 "arg1"
    // | char* argv[0]         | --> 指向上面的 "./stack_layout"
    // +-----------------------+
    // | int argc              | // 实际上在 a0 寄存器中
    // +-----------------------+
    // | Return Address        |
    // +-----------------------+     <--- main 函数的栈帧开始
    // （低地址 - 栈顶）
    pub fn execve(
        &mut self,
        new_memory_space: Arc<MemorySpace>,
        entry_point: usize,
        sp_high: usize, // 新栈的最高地址
        argv: &[&str],
        envp: &[&str],
    ) {
        // 1. 准备返回到 U 模式
        let mut sstatus_val = sstatus::read();
        sstatus_val.set_spp(sstatus::SPP::User);
        sstatus_val.set_sie(false);
        sstatus_val.set_spie(true);

        // 2. 切换任务的地址空间对象
        self.memory_space = Some(new_memory_space);

        let tf_ptr = self.trap_frame_ptr.load(Ordering::SeqCst);

        let mut arg_ptrs: Vec<usize> = Vec::with_capacity(argv.len());
        let mut env_ptrs: Vec<usize> = Vec::with_capacity(envp.len());
        let mut current_sp = sp_high;

        // 注意：以下拷贝时对sp进行的操作均要求已经可以访问用户栈空间
        //      也就是说，new_memory_space 已经被激活（切换 satp）
        //      否则必须实现类似 copy_to_user 的函数来完成拷贝,不然会引发页错误

        // --- 拷贝字符串数据 (从高地址向低地址压栈) ---

        println!("current_sp before copy: {:#x}", current_sp);
        // 在S态写用户页前，临时开启 SUM
        let prev_sum = sstatus::read().sum();
        unsafe {
            // 若 riscv crate 提供该API，请使用它；否则参见下面的内联汇编备选
            sstatus::set_sum();
        }

        // 环境变量 (envp)
        for &env in envp.iter().rev() {
            let bytes = env.as_bytes();
            current_sp -= bytes.len() + 1; // 预留 NUL
            unsafe {
                ptr::copy_nonoverlapping(bytes.as_ptr(), current_sp as *mut u8, bytes.len());
                (current_sp as *mut u8).add(bytes.len()).write(0); // NUL 终止符
            }
            env_ptrs.push(current_sp); // 存储字符串的地址
        }

        // 命令行参数 (argv)
        for &arg in argv.iter().rev() {
            let bytes = arg.as_bytes();
            current_sp -= bytes.len() + 1; // 预留 NUL
            unsafe {
                ptr::copy_nonoverlapping(bytes.as_ptr(), current_sp as *mut u8, bytes.len());
                (current_sp as *mut u8).add(bytes.len()).write(0); // NUL 终止符
            }
            arg_ptrs.push(current_sp); // 存储字符串的地址
        }

        // --- 对齐到字大小 (确保指针数组从对齐的地址开始) ---
        current_sp &= !(size_of::<usize>() - 1);

        // --- 构建 argc, argv, envp 数组 (ABI 标准布局: [argc] -> [argv] -> [NULL] -> [envp] -> [NULL]) ---
        // 注意：栈向下增长，所以压栈顺序是从 envp NULL 往回压到 argc

        // 1. 写入 envp NULL 终止符
        current_sp -= size_of::<usize>();
        unsafe {
            ptr::write(current_sp as *mut usize, 0);
        }

        // 2. 写入 envp 指针数组（逆序写入，使 envp[0] 处于最低地址）
        // env_ptrs 已经是逆序 (envp[n-1] ... envp[0])
        for &p in env_ptrs.iter() {
            current_sp -= size_of::<usize>();
            unsafe {
                ptr::write(current_sp as *mut usize, p);
            }
        }
        let envp_vec_ptr = current_sp; // envp 数组的起始地址 (envp[0] 的地址)

        // 3. 写入 argv NULL 终止符
        current_sp -= size_of::<usize>();
        unsafe {
            ptr::write(current_sp as *mut usize, 0);
        }

        // 4. 写入 argv 指针数组（逆序写入，使 argv[0] 处于最低地址）
        // arg_ptrs 已经是逆序 (argv[n-1] ... argv[0])
        for &p in arg_ptrs.iter() {
            current_sp -= size_of::<usize>();
            unsafe {
                ptr::write(current_sp as *mut usize, p);
            }
        }
        let argv_vec_ptr = current_sp; // argv 数组的起始地址 (argv[0] 的地址)

        // 5. 写入 argc
        let argc = argv.len();
        current_sp -= size_of::<usize>();
        unsafe {
            ptr::write(current_sp as *mut usize, argc);
        }

        // 拷贝完成，恢复 SUM
        unsafe {
            sstatus::clear_sum();
        }

        // 6. 最终 16 字节对齐（应用到最终栈指针 current_sp）
        current_sp &= !STACK_ALIGN_MASK;
        println!("current_sp after copy: {:#x}", current_sp);
        // 4. 配置 TrapFrame (新的上下文)
        unsafe {
            // 清零整个 TrapFrame，避免旧值泄漏到用户态
            core::ptr::write_bytes(tf_ptr, 0, 1);

            // 设置用户陷入内核时使用的内核栈指针
            (*tf_ptr).kernel_sp = self.kstack_base;

            // 设置程序执行的入口地址 (PC)
            (*tf_ptr).sepc = entry_point;

            // 设置权限状态 SSTATUS
            (*tf_ptr).sstatus = sstatus_val.bits();

            // 用户栈指针 (最终的 16 字节对齐地址)
            (*tf_ptr).x2_sp = current_sp;

            // main(argc, argv, envp) 约定：a0=argc, a1=argv, a2=envp
            (*tf_ptr).x10_a0 = argc;
            (*tf_ptr).x11_a1 = argv_vec_ptr;
            (*tf_ptr).x12_a2 = envp_vec_ptr;

            // 清零 ra，避免意外返回路径，用户态程序应通过正常的退出机制结束
            (*tf_ptr).x1_ra = 0;
        }
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
        // let kstack_top = kstack_tracker.start_ppn().start_addr().to_vaddr().as_usize();
        // let trap_end = trap_frame_tracker.ppn().end_addr().to_vaddr().as_usize();
        // println!(
        //     "Task::new: tid={}, ppid={}, kstack: [{:#x} - {:#x}], trap_frame: [{:#x} - {:#x}]",
        //     tid, ppid, kstack_top, kstack_base, trap_frame_ptr, trap_end
        // );
        // 简单的 guard, 向TrapFrame所在页末位写入一个值，以防止越界访问
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

    // 创建内核任务的基本属性检查
    test_case!(test_ktask_create, {
        println!("Testing: test_ktask_create");
        let kstack_tracker =
            physical_page_alloc_contiguous(4).expect("kthread_spawn: failed to alloc kstack");
        let trap_frame_tracker =
            physical_page_alloc().expect("kthread_spawn: failed to alloc trap_frame");
        let t = Task::ktask_create(1, 1, 0, kstack_tracker, trap_frame_tracker, 0x1000);
        // tid/ pid 应有效且相等
        kassert!(t.tid != 0);
        kassert!(t.pid == t.tid);
        // 默认创建为内核线程（memory_space == None）
        kassert!(t.is_kernel_thread());
        // 内核栈基址应非零
        kassert!(t.kstack_base != 0);
    });
}
