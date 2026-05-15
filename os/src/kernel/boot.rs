//! 架构无关的启动代码
//!
//! 包含内核早期初始化、init/idle 任务创建等所有架构共享的启动逻辑。
//! 新架构移植时只需实现 arch::boot::main() 和 idle_loop()，其余自动复用。

use alloc::sync::Arc;

use crate::{
    arch::CpuOps,
    ipc::{SignalHandlerTable, SignalPending},
    kernel::{
        FsStruct, Scheduler, TASK_MANAGER, TaskManagerTrait, TaskStruct, current_memory_space,
        current_task, kernel_execve, kthread_spawn, kworker, scheduler_of, sleep_task_with_block,
        yield_task,
    },
    mm::frame_allocator::{alloc_contig_frames, alloc_frame},
    pr_err, pr_info, pr_warn,
    sync::SpinLock,
    uapi::{
        resource::{INIT_RLIMITS, RlimitStruct},
        signal::SignalFlags,
        uts_namespace::UtsNamespace,
    },
    vfs::{create_stdio_files, fd_table::FDTable, get_root_dentry},
};

/// 架构无关的 idle 循环
///
/// 确保中断开启后持续等待中断，唤醒后立即重新等待。
/// 使用 `ArchImpl::halt()` 执行具体的 halt 指令（wfi / idle 0）。
pub fn idle_loop() -> ! {
    loop {
        if !crate::arch::intr::are_interrupts_enabled() {
            unsafe {
                crate::arch::intr::enable_interrupts();
            }
        }
        crate::arch::ArchImpl::halt();
    }
}

/// 清除 BSS 段
///
/// 将 BSS 段全部置零。通过 Arch 的地址翻译方法访问物理内存。
pub fn clear_bss() {
    unsafe extern "C" {
        fn sbss();
        fn ebss();
    }

    let sbss_paddr = unsafe { crate::arch::vaddr_to_paddr(sbss as usize) };
    let ebss_paddr = unsafe { crate::arch::vaddr_to_paddr(ebss as usize) };

    (sbss_paddr..ebss_paddr).for_each(|a| unsafe {
        let va = crate::arch::paddr_to_vaddr(a);
        (va as *mut u8).write_volatile(0)
    });
}

/// 创建 init 任务 (PID=1) 并加入调度队列
///
/// 调用后 init 任务在运行队列中就绪，调用者应进入 idle_loop 等待
/// 下一次时钟中断触发调度，由调度器自动选中 init 并切换上下文。
pub fn rest_init() {
    let tid = 1;
    let kstack_tracker = alloc_contig_frames(4).expect("rest_init: failed to alloc kstack");
    let trap_frame_tracker = alloc_frame().expect("rest_init: failed to alloc trap_frame");
    let fd_table = FDTable::new();
    let (stdin, stdout, stderr) = create_stdio_files();
    fd_table
        .install_at(0, stdin)
        .expect("Failed to install stdin");
    fd_table
        .install_at(1, stdout)
        .expect("Failed to install stdout");
    fd_table
        .install_at(2, stderr)
        .expect("Failed to install stderr");
    let cwd = get_root_dentry().ok();
    let root = cwd.clone();
    let fs = Arc::new(SpinLock::new(FsStruct::new(cwd, root)));
    let mut task = TaskStruct::ktask_create(
        tid,
        tid,
        0,
        TaskStruct::empty_children(),
        kstack_tracker,
        trap_frame_tracker,
        Arc::new(SpinLock::new(SignalHandlerTable::new())),
        SignalFlags::empty(),
        Arc::new(SpinLock::new(SignalPending::empty())),
        Arc::new(SpinLock::new(UtsNamespace::default())),
        Arc::new(SpinLock::new(RlimitStruct::new(INIT_RLIMITS))),
        Arc::new(fd_table),
        fs,
    );

    let tf = task
        .trap_frame_ptr
        .load(core::sync::atomic::Ordering::SeqCst);
    unsafe {
        core::ptr::write(tf, crate::arch::trap::TrapFrame::zero_init());
        (*tf).set_kernel_trap_frame(init as usize, 0, task.kstack_base);
    }

    task.memory_space = Some(current_memory_space());
    task.on_cpu = Some(0);
    let task = task.into_shared();

    TASK_MANAGER.lock().add_task(task.clone());
    scheduler_of(0).lock().add_task(task);
}

/// PID = 1: 完成剩余初始化，然后 exec /sbin/init
///
/// 此时 trap::init() 和 enable_interrupts() 已在 main() 中完成，
/// 调度器正常运行，本函数作为 init 任务入口在 forkret 之后被调度执行。
fn init() {
    create_kthreadd();

    if let Err(e) = crate::fs::init_ext4_from_block_device() {
        pr_err!(
            "[Init] Warning: Failed to initialize Ext4 filesystem: {:?}",
            e
        );
        pr_info!("[Init] Continuing without filesystem...");
    }

    if let Err(e) = crate::net::config::NetworkConfigManager::init_default_interface() {
        pr_warn!(
            "[Init] Warning: Failed to init default network interface: {:?}",
            e
        );
    }

    kernel_execve("/sbin/init", &["/sbin/init"], &[]);
}

/// 内核守护线程 PID = 2
///
/// 负责创建内核任务，回收僵尸任务等工作
fn kthreadd() {
    kthread_spawn(kworker);
    loop {
        sleep_task_with_block(current_task(), true);
        yield_task();
    }
}

/// 创建 kthreadd 任务 (PID=2)
fn create_kthreadd() {
    let tid = TASK_MANAGER.lock().allocate_tid();
    let kstack_tracker = alloc_contig_frames(4).expect("create_kthreadd: failed to alloc kstack");
    let trap_frame_tracker = alloc_frame().expect("create_kthreadd: failed to alloc trap_frame");
    let (uts, rlimit, fd_table, fs) = {
        let task = current_task();
        let t = task.lock();
        (
            t.uts_namespace.clone(),
            t.rlimit.clone(),
            t.fd_table.clone_table(),
            t.fs.lock().clone(),
        )
    };
    let task = TaskStruct::ktask_create(
        tid,
        tid,
        0,
        TaskStruct::empty_children(),
        kstack_tracker,
        trap_frame_tracker,
        Arc::new(SpinLock::new(SignalHandlerTable::new())),
        SignalFlags::empty(),
        Arc::new(SpinLock::new(SignalPending::empty())),
        uts,
        rlimit,
        Arc::new(fd_table),
        Arc::new(SpinLock::new(fs)),
    );

    let tf = task
        .trap_frame_ptr
        .load(core::sync::atomic::Ordering::SeqCst);
    unsafe {
        core::ptr::write(tf, crate::arch::trap::TrapFrame::zero_init());
        (*tf).set_kernel_trap_frame(kthreadd as usize, 0, task.kstack_base);
    }
    let task = task.into_shared();
    TASK_MANAGER.lock().add_task(task.clone());
    task.lock().on_cpu = Some(0);
    scheduler_of(0).lock().add_task(task);
}

/// 为指定 CPU 创建 idle 任务
///
/// idle 任务使用 `idle_fn` 作为入口（各架构自行提供 wfi/idle 0 循环）。
pub fn create_idle_task(cpu_id: usize, idle_fn: fn() -> !) -> crate::kernel::SharedTask {
    let tid = TASK_MANAGER.lock().allocate_tid();
    let kstack_tracker =
        alloc_contig_frames(1).expect("Failed to allocate kernel stack for idle task");
    let trap_frame_tracker = alloc_frame().expect("Failed to allocate trap frame for idle task");

    let mut task = TaskStruct::ktask_create(
        tid,
        tid,
        0,
        TaskStruct::empty_children(),
        kstack_tracker,
        trap_frame_tracker,
        Arc::new(SpinLock::new(SignalHandlerTable::new())),
        SignalFlags::empty(),
        Arc::new(SpinLock::new(SignalPending::empty())),
        Arc::new(SpinLock::new(UtsNamespace::default())),
        Arc::new(SpinLock::new(RlimitStruct::new(INIT_RLIMITS))),
        Arc::new(FDTable::new()),
        Arc::new(SpinLock::new(FsStruct::new(None, None))),
    );

    let tf = task
        .trap_frame_ptr
        .load(core::sync::atomic::Ordering::SeqCst);
    unsafe {
        core::ptr::write(tf, crate::arch::trap::TrapFrame::zero_init());
        (*tf).set_kernel_trap_frame(idle_fn as usize, 0, task.kstack_base);
    }

    task.on_cpu = Some(cpu_id);
    let task = task.into_shared();
    TASK_MANAGER.lock().add_task(task.clone());

    pr_info!("[SMP] Created idle task {} for CPU {}", tid, cpu_id);

    task
}
