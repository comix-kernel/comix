//! LoongArch64 架构启动代码
//!
//! 启动流程：
//! 1. entry.S 配置 DMW 并跳转到虚拟地址
//! 2. rust_main 调用 main() 进行初始化
//! 3. 初始化内存管理、中断、定时器等子系统
//! 4. 创建第一个任务并开始调度

use core::{arch::global_asm, sync::atomic::Ordering};

use alloc::sync::Arc;

use crate::{
    arch::{intr, platform, timer, trap},
    earlyprintln,
    ipc::{SignalHandlerTable, SignalPending},
    kernel::{
        FsStruct, SCHEDULER, Scheduler, TASK_MANAGER, TaskManagerTrait, TaskStruct, current_cpu,
        current_memory_space, current_task, kernel_execve, kthread_spawn, kworker,
        sleep_task_with_block, time, yield_task,
    },
    mm::{
        self,
        frame_allocator::{alloc_contig_frames, alloc_frame},
    },
    pr_err, pr_info,
    sync::SpinLock,
    test::run_early_tests,
    uapi::{
        resource::{INIT_RLIMITS, RlimitStruct},
        signal::SignalFlags,
        uts_namespace::UtsNamespace,
    },
    vfs::{create_stdio_files, fd_table, get_root_dentry},
};

global_asm!(include_str!("entry.S"));

/// 内核的第一个任务启动函数
pub fn rest_init() -> ! {
    let tid = TASK_MANAGER.lock().allocate_tid();
    let kstack_tracker = alloc_contig_frames(4).expect("kthread_spawn: failed to alloc kstack");
    let trap_frame_tracker = alloc_frame().expect("kthread_spawn: failed to alloc trap_frame");
    let fd_table = fd_table::FDTable::new();
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

    let tf = task.trap_frame_ptr.load(Ordering::SeqCst);
    unsafe {
        (*tf).set_kernel_trap_frame(init as usize, 0, task.kstack_base);
    }

    let ra = task.context.ra;
    let sp = task.context.sp;
    let ptr = task.trap_frame_ptr.load(Ordering::SeqCst);
    task.memory_space = Some(current_memory_space());
    let task = task.into_shared();
    crate::arch::trap::set_trap_frame_ptr(ptr as usize);
    TASK_MANAGER.lock().add_task(task.clone());
    current_cpu().lock().switch_task(task);

    unsafe {
        core::arch::asm!(
            "add.d $sp, {sp}, $r0",
            "jirl $r0, {ra}, 0",
            sp = in(reg) sp,
            ra = in(reg) ra,
            options(noreturn)
        );
    }
}

fn init() {
    trap::init();

    create_kthreadd();

    if let Err(e) = crate::fs::init_ext4_from_block_device() {
        pr_err!(
            "[Init] Warning: Failed to initialize Ext4 filesystem: {:?}",
            e
        );
        pr_info!("[Init] Continuing without filesystem...");
    }

    kernel_execve("/sbin/init", &["/sbin/init"], &[]);
}

fn kthreadd() {
    kthread_spawn(kworker);
    loop {
        sleep_task_with_block(current_task(), true);
        yield_task();
    }
}

fn create_kthreadd() {
    let tid = TASK_MANAGER.lock().allocate_tid();
    let kstack_tracker = alloc_contig_frames(4).expect("kthread_spawn: failed to alloc kstack");
    let trap_frame_tracker = alloc_frame().expect("kthread_spawn: failed to alloc trap_frame");
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

    let tf = task.trap_frame_ptr.load(Ordering::SeqCst);
    unsafe {
        (*tf).set_kernel_trap_frame(kthreadd as usize, 0, task.kstack_base);
    }
    let task = task.into_shared();
    TASK_MANAGER.lock().add_task(task.clone());
    SCHEDULER.lock().add_task(task);
}

/// LoongArch 架构初始化入口
/// 由 main.rs 中的 rust_main 调用
pub fn main(_hartid: usize) -> ! {
    clear_bss();

    run_early_tests();

    earlyprintln!("[Boot] Hello, LoongArch!");
    earlyprintln!("[Boot] LoongArch64 kernel is starting...");

    mm::init();

    #[cfg(test)]
    crate::test_main();

    trap::init_boot_trap();
    platform::init();
    time::init();
    timer::init();
    unsafe { intr::enable_interrupts() };

    rest_init()
}

/// 清除 BSS 段，将其全部置零
/// BSS 段包含所有未初始化的静态变量
/// 在进入 Rust 代码之前调用此函数非常重要
///
/// 注意：此时已在虚拟地址空间运行，sbss/ebss 是虚拟地址
fn clear_bss() {
    unsafe extern "C" {
        fn sbss();
        fn ebss();
    }

    // sbss 和 ebss 已经是虚拟地址，DMW 映射后可直接访问
    unsafe {
        let start = sbss as *mut u8;
        let end = ebss as *mut u8;
        let len = end.offset_from(start) as usize;
        core::slice::from_raw_parts_mut(start, len).fill(0);
    }
}
