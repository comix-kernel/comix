use core::{hint, sync::atomic::Ordering};

use alloc::sync::Arc;
use riscv::register::sscratch;

use crate::{
    arch::trap::{self, restore},
    fs::ROOT_FS,
    kernel::{
        SCHEDULER, TaskState,
        cpu::current_cpu,
        scheduler::Scheduler,
        task::{TASK_MANAGER, TaskStruct, into_shared},
    },
    mm::{
        activate,
        frame_allocator::{physical_page_alloc, physical_page_alloc_contiguous},
        memory_space::MemorySpace,
    },
};

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
    let tid = TASK_MANAGER.lock().allocate_tid();
    let (pid, ppid) = {
        let cur_cpu = current_cpu().lock();
        let cur_task = cur_cpu.current_task.as_ref().unwrap();
        let cur_task = cur_task.lock();
        (cur_task.pid, cur_task.ppid)
    };
    let kstack_tracker =
        physical_page_alloc_contiguous(4).expect("kthread_spawn: failed to alloc kstack");
    let trap_frame_tracker =
        physical_page_alloc().expect("kthread_spawn: failed to alloc trap_frame");

    // 分配 Task 结构体和内核栈
    let task = TaskStruct::ktask_create(
        tid,
        pid,
        ppid,
        kstack_tracker,
        trap_frame_tracker,
        entry_addr,
    );
    let tid = task.tid;
    let task = into_shared(task);

    // 将任务加入调度器和任务管理器
    TASK_MANAGER.lock().add_task(task.clone());
    SCHEDULER.lock().add_task(task);

    tid
}

/// 等待指定 tid 的任务结束
/// 该函数会阻塞调用者直到目标任务状态变为 Stopped
/// 如果目标任务不存在则立即返回错误码
/// 任务结束后会将其从任务管理器中移除
/// 并将其返回值写入调用者提供的指针地址
/// # 参数
/// * `tid`: 目标任务的任务 ID
/// * `return_value_ptr`: 用于存放目标任务返回值的指针
/// # 返回值
/// 成功返回 0，失败返回 -1
pub fn kthread_join(tid: u32, return_value_ptr: Option<usize>) -> i32 {
    loop {
        let task_opt = TASK_MANAGER.lock().get_task(tid);
        if let Some(task) = task_opt {
            let t = task.lock();
            if t.state == TaskState::Stopped {
                if let Some(rv) = t.return_value {
                    unsafe {
                        if let Some(ptr) = return_value_ptr {
                            let ptr = ptr as *mut usize;
                            ptr.write_volatile(rv);
                        }
                    }
                }
                TASK_MANAGER.lock().remove_task(tid);
                return 0; // 成功结束
            }
        } else {
            return -1; // 任务不存在，直接返回
        }
        // 暂时的忙等待
        hint::spin_loop();
    }
}

/// 在内核任务中执行 execve，加载并运行指定路径的 ELF 可执行文件
/// 该函数不会返回，执行成功后会切换到新程序的入口点
/// # 参数
/// * `path`: ELF 可执行文件的路径
/// * `argv`: 传递给新程序的参数列表
/// * `envp`: 传递给新程序的环境变量列表
pub fn kernel_execve(path: &str, argv: &[&str], envp: &[&str]) -> ! {
    let data = ROOT_FS
        .load_elf(path)
        .expect("kernel_execve: file not found");

    let (space, entry, sp) =
        MemorySpace::from_elf(data).expect("kernel_execve: failed to create memory space from ELF");
    let space: Arc<MemorySpace> = Arc::new(space);
    // 换掉当前任务的地址空间，e.g. 切换 satp
    activate(space.root_ppn());

    let cpu = current_cpu().lock();
    let task = cpu.current_task.as_ref().unwrap().clone();

    {
        let mut t = task.lock();
        t.execve(space, entry, sp, argv, envp);
    }

    let tfp = task.lock().trap_frame_ptr.load(Ordering::SeqCst);
    // sscratch 将在__restore中恢复，指向当前任务的 TrapFrame
    // 直接按 trapframe 状态恢复并 sret 到用户态
    unsafe {
        restore(&*tfp);
    }
    unreachable!("kernel_execve: should not return");
}

/// 内核的第一个任务启动函数
/// 并且当这个函数结束时，应该切换到第一个任务的上下文
pub fn rest_init() {
    let tid = TASK_MANAGER.lock().allocate_tid();
    let kstack_tracker =
        physical_page_alloc_contiguous(4).expect("kthread_spawn: failed to alloc kstack");
    let trap_frame_tracker =
        physical_page_alloc().expect("kthread_spawn: failed to alloc trap_frame");
    let task = into_shared(TaskStruct::ktask_create(
        tid,
        tid,
        0,
        kstack_tracker,
        trap_frame_tracker,
        init as usize,
    )); // init 没有父任务

    let (ra, sp) = {
        let g = task.lock();
        let ra = g.context.ra;
        let sp = g.context.sp;
        (ra, sp)
    };

    let ptr = task.lock().trap_frame_ptr.load(Ordering::SeqCst);
    unsafe {
        sscratch::write(ptr as usize);
    }
    current_cpu().lock().current_task = Some(task);

    // 切入 kinit：设置 sp 并跳到 ra；此调用不返回
    unsafe {
        core::arch::asm!(
            "mv sp, {sp}",
            "jr {ra}",
            sp = in(reg) sp,
            ra = in(reg) ra,
            options(noreturn)
        );
    }
}

/// 内核的第一个任务
/// PID = 1
/// 负责进行剩余的初始化工作
/// 创建 kthreadd 任务
/// 并在一切结束后转化为第一个用户态任务
fn init() {
    trap::init();
    create_kthreadd();
    kernel_execve("hello", &["hello"], &[]);
}

/// 内核守护线程
/// PID = 2
/// 负责创建内核任务，回收僵尸任务等工作
fn kthreadd() {
    loop {
        hint::spin_loop();
    }
}

/// 创建内核守护线程 kthreadd
fn create_kthreadd() {
    let tid = TASK_MANAGER.lock().allocate_tid();
    let kstack_tracker =
        physical_page_alloc_contiguous(4).expect("kthread_spawn: failed to alloc kstack");
    let trap_frame_tracker =
        physical_page_alloc().expect("kthread_spawn: failed to alloc trap_frame");
    let task = into_shared(TaskStruct::ktask_create(
        tid,
        tid,
        0,
        kstack_tracker,
        trap_frame_tracker,
        kthreadd as usize,
    )); // kthreadd 没有父任务

    TASK_MANAGER.lock().add_task(task.clone());
    SCHEDULER.lock().add_task(task);
}
