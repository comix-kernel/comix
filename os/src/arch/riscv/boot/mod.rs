//! RISC-V 架构相关的启动代码

use core::sync::atomic::Ordering;

use alloc::sync::Arc;
use riscv::register::sscratch;

use crate::{
    arch::{intr, mm::vaddr_to_paddr, platform, timer, trap},
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
    pr_err, pr_info, println,
    sync::SpinLock,
    test::run_early_tests,
    uapi::{
        resource::{INIT_RLIMITS, RlimitStruct},
        signal::SignalFlags,
        uts_namespace::UtsNamespace,
    },
    vfs::{create_stdio_files, fd_table, get_root_dentry},
};

/// 内核的第一个任务启动函数
/// 并且当这个函数结束时，应该切换到第一个任务的上下文
pub fn rest_init() {
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
    ); // init 没有父任务

    let tf = task.trap_frame_ptr.load(Ordering::SeqCst);
    // Safety: 此时 trap_frame_tracker 已经分配完毕且不可变更，所有权在 task 中，指针有效
    unsafe {
        (*tf).set_kernel_trap_frame(init as usize, 0, task.kstack_base);
    }

    let ra = task.context.ra;
    let sp = task.context.sp;
    let ptr = task.trap_frame_ptr.load(Ordering::SeqCst);
    // init 进程不同于其他内核线程，需要有一个独立的内存空间
    task.memory_space = Some(current_memory_space());
    let task = task.into_shared();
    unsafe {
        sscratch::write(ptr as usize);
    }
    TASK_MANAGER.lock().add_task(task.clone());
    current_cpu().lock().switch_task(task);

    // 切入 kinit：设置 sp 并跳到 ra；此调用不返回
    // SAFETY: 在 Task 创建时已正确初始化 ra 和 sp
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
    super::trap::init();

    create_kthreadd();

    // 初始化 Ext4 文件系统（从真实块设备）
    // 必须在任务上下文中进行,因为 VFS 需要 current_task()
    if let Err(e) = crate::fs::init_ext4_from_block_device() {
        pr_err!(
            "[Init] Warning: Failed to initialize Ext4 filesystem: {:?}",
            e
        );
        pr_info!("[Init] Continuing without filesystem...");
    }

    // // 挂载 /dev 并创建设备节点
    // if let Err(e) = crate::fs::mount_tmpfs("/dev", 0) {
    //     pr_err!("[Init] Failed to mount /dev: {:?}", e);
    // } else if let Err(e) = crate::fs::init_dev() {
    //     pr_err!("[Init] Failed to create devices: {:?}", e);
    // }

    kernel_execve("/sbin/init", &["/sbin/init"], &[]);
}

/// 内核守护线程
/// PID = 2
/// 负责创建内核任务，回收僵尸任务等工作
fn kthreadd() {
    kthread_spawn(kworker);
    loop {
        // 休眠等待任务
        sleep_task_with_block(current_task(), true);
        yield_task();
    }
}

/// 创建内核守护线程 kthreadd
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
    ); // kthreadd 没有父任务

    let tf = task.trap_frame_ptr.load(Ordering::SeqCst);
    // Safety: 此时 trap_frame_tracker 已经分配完毕且不可变更，所有权在 task 中，指针有效
    unsafe {
        (*tf).set_kernel_trap_frame(kthreadd as usize, 0, task.kstack_base);
    }
    let task = task.into_shared();
    TASK_MANAGER.lock().add_task(task.clone());
    SCHEDULER.lock().add_task(task);
}

pub fn main(hartid: usize) {
    clear_bss();

    run_early_tests();

    earlyprintln!("[Boot] Hello, world!");
    earlyprintln!("[Boot] RISC-V Hart {} is up!", hartid);

    mm::init();

    #[cfg(test)]
    crate::test_main();

    // 初始化工作
    trap::init_boot_trap();
    platform::init();
    time::init();
    timer::init();
    unsafe { intr::enable_interrupts() };

    rest_init();
}

/// 清除 BSS 段，将其全部置零
/// BSS 段包含所有未初始化的静态变量
/// 在进入 Rust 代码之前调用此函数非常重要
fn clear_bss() {
    unsafe extern "C" {
        fn sbss();
        fn ebss();
    }

    let sbss_paddr = unsafe { vaddr_to_paddr(sbss as usize) };
    let ebss_paddr = unsafe { vaddr_to_paddr(ebss as usize) };

    (sbss_paddr..ebss_paddr).for_each(|a| unsafe {
        // 访问物理地址需要通过 paddr_to_vaddr 转换
        let va = crate::arch::mm::paddr_to_vaddr(a);
        (va as *mut u8).write_volatile(0)
    });
}

// 由于最近的更新使得create_kthreadd内部会调用current_task等函数
// 该单元测试已无法在不完整的测试环境下运行
// #[cfg(test)]
// mod tests {

//     use core::sync::atomic::Ordering;

//     // 测试 create_kthreadd：应创建一个任务并加入 TASK_MANAGER
//     use crate::{
//         arch::boot::{create_kthreadd, kthreadd},
//         kassert,
//         kernel::{TASK_MANAGER, TaskManagerTrait},
//         test_case,
//     };

//     test_case!(test_create_kthreadd, {
//         // 记录当前已有任务数量
//         let before_count = {
//             let mgr = TASK_MANAGER.lock();
//             mgr.task_count()
//         };
//         create_kthreadd();
//         // 找到新增的任务（PID=tid，入口=kthreadd）
//         let after_count = {
//             let mgr = TASK_MANAGER.lock();
//             mgr.task_count()
//         };
//         kassert!(after_count == before_count + 1);
//         // 查找新 tid
//         let new_tid = after_count as u32; // 简单假设 tid 连续分配
//         let task = TASK_MANAGER.lock().get_task(new_tid).expect("task missing");
//         let g = task.lock();
//         let tf = g.trap_frame_ptr.load(Ordering::SeqCst);
//         kassert!(g.tid == new_tid);
//         kassert!(g.pid == new_tid); // kthreadd 设 pid=tid
//         kassert!(unsafe { (*tf).sepc } as usize == kthreadd as usize);
//     });

//     // 由于 kernel_execve / rest_init / init / kthreadd 涉及不可返回的流控与实际陷入/页表切换，
//     // 在单元测试环境下不执行它们（需要集成测试或仿真环境）。
// }
