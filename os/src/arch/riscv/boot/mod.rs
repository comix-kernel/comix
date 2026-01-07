//! RISC-V 架构相关的启动代码

use core::arch::global_asm;
use core::sync::atomic::{AtomicUsize, Ordering};

use alloc::sync::Arc;
use riscv::register::sscratch;

global_asm!(include_str!("entry.S"));

use crate::{
    arch::{intr, mm::vaddr_to_paddr, platform, timer, trap, trap::TrapFrame},
    earlyprintln,
    ipc::{SignalHandlerTable, SignalPending},
    kernel::{
        FsStruct, NUM_CPU, Scheduler, TASK_MANAGER, TaskManagerTrait, TaskStruct, current_cpu,
        current_memory_space, current_task, kernel_execve, kthread_spawn, kworker,
        sleep_task_with_block, time, yield_task,
    },
    mm::{
        self,
        frame_allocator::{alloc_contig_frames, alloc_frame},
    },
    pr_debug, pr_err, pr_info, pr_warn,
    sync::SpinLock,
    test::run_early_tests,
    uapi::{
        resource::{INIT_RLIMITS, RlimitStruct},
        signal::SignalFlags,
        uts_namespace::UtsNamespace,
    },
    vfs::{create_stdio_files, fd_table, get_root_dentry},
};
// Needed for Ppn::as_usize
use crate::mm::address::UsizeConvert;

/// 已上线 CPU 位掩码
///
/// 每个位代表一个 CPU，位 i 为 1 表示 CPU i 已上线。
/// 使用原子操作确保多核环境下的线程安全。
static CPU_ONLINE_MASK: AtomicUsize = AtomicUsize::new(0);

/// 从核启动标志（在 entry.S 中定义）
///
/// 主核设置此标志为 1 后，所有从核将从 WFI 中唤醒并开始启动。
unsafe extern "C" {
    static mut secondary_boot_flag: u64;
}

/// 从核调试入口（在启用分页后立即调用）
#[unsafe(no_mangle)]
pub extern "C" fn secondary_debug_entry(hartid: usize) {
    crate::earlyprintln!("[DEBUG] Hart {} reached secondary_wait_high", hartid);
}

/// 内核的第一个任务启动函数
/// 并且当这个函数结束时，应该切换到第一个任务的上下文
pub fn rest_init() {
    // init进程必须使用TID 1，不从分配器获取
    // TID分配器从2开始，所以idle任务会获得TID 2, 3, ...
    let tid = 1;
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
        // 先初始化 TrapFrame 为全 0
        core::ptr::write(tf, TrapFrame::zero_init());
        (*tf).set_kernel_trap_frame(init as usize, 0, task.kstack_base);
    }

    let ra = task.context.ra;
    let sp = task.context.sp;
    let ptr = task.trap_frame_ptr.load(Ordering::SeqCst);
    // init 进程不同于其他内核线程，需要有一个独立的内存空间
    task.memory_space = Some(current_memory_space());
    // init 任务运行在 CPU 0
    task.on_cpu = Some(0);
    let task = task.into_shared();
    unsafe {
        sscratch::write(ptr as usize);
    }
    TASK_MANAGER.lock().add_task(task.clone());
    {
        let _guard = crate::sync::PreemptGuard::new();
        current_cpu().switch_task(task);
    }

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

    // 启用中断（在设置好 trap 处理和 sscratch 之后）
    unsafe { intr::enable_interrupts() };

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

    // 初始化默认网络配置（eth0 + 127.0.0.1 loopback + 全局 NET_IFACE）
    if let Err(e) = crate::net::config::NetworkConfigManager::init_default_interface() {
        pr_warn!(
            "[Init] Warning: Failed to init default network interface: {:?}",
            e
        );
    }

    // /dev 的挂载与设备节点创建交给用户态 rcS：
    // - rcS 会执行 `mount -t tmpfs none /dev`
    // - 内核在 mount("/dev") 的系统调用中对该挂载点做了特殊处理，会在挂载 tmpfs 后自动 init_dev()

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
        // 先初始化 TrapFrame 为全 0
        core::ptr::write(tf, TrapFrame::zero_init());
        (*tf).set_kernel_trap_frame(kthreadd as usize, 0, task.kstack_base);
    }
    let task = task.into_shared();
    // kthreadd 任务运行在 CPU 0
    task.lock().on_cpu = Some(0);
    TASK_MANAGER.lock().add_task(task.clone());
    crate::kernel::scheduler_of(0).lock().add_task(task);
}

pub fn main(hartid: usize) {
    clear_bss();

    run_early_tests();

    earlyprintln!("[Boot] Hello, world!");
    earlyprintln!("[Boot] RISC-V Hart {} is up!", hartid);

    let kernel_space = mm::init();

    // 初始化 CPUS 并设置 tp 指向 CPU 0
    // 必须在任何可能调用 cpu_id() 的代码之前完成
    {
        use crate::kernel::CPUS;
        let cpu_ptr = &*CPUS.get_of(0) as *const _ as usize;
        unsafe {
            core::arch::asm!("mv tp, {}", in(reg) cpu_ptr);
        }
        earlyprintln!("[Boot] Initialized CPUS, tp = 0x{:x}", cpu_ptr);
    }

    // 激活内核地址空间并设置 current_memory_space
    {
        let _guard = crate::sync::PreemptGuard::new();
        current_cpu().switch_space(kernel_space);
        earlyprintln!("[Boot] Activated kernel address space");
    }

    #[cfg(test)]
    crate::test_main();

    // 初始化工作
    trap::init_boot_trap();
    platform::init(); // 完整的平台初始化 (包括 device_tree::init())
    time::init();

    // 启动从核（在启用定时器中断之前）
    let num_cpus = unsafe { NUM_CPU };
    if num_cpus > 1 {
        boot_secondary_cpus(num_cpus);
    }

    // 在从核启动完成后再初始化定时器，避免主核在等待时收到中断
    timer::init();

    // 为 CPU0 创建并登记 idle 任务（不加入调度队列，仅作兜底）
    {
        let _guard = crate::sync::PreemptGuard::new();
        if current_cpu().idle_task.is_none() {
            let idle0 = create_idle_task(0);
            current_cpu().idle_task = Some(idle0);
        }
    }

    // 注意：中断在 init() 函数中启用，在设置好 sscratch 之后
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{kassert, test_case};

    /// 测试 NUM_CPU 设置正确
    test_case!(test_num_cpu, {
        let num_cpu = unsafe { crate::kernel::NUM_CPU };
        kassert!(num_cpu >= 1);
        kassert!(num_cpu <= crate::config::MAX_CPU_COUNT);
    });

    /// 测试 CPU 上线掩码（多核环境）
    test_case!(test_cpu_online_mask, {
        use core::sync::atomic::Ordering;

        let num_cpu = unsafe { crate::kernel::NUM_CPU };
        let actual_mask = CPU_ONLINE_MASK.load(Ordering::Acquire);

        // 在测试模式下，如果 CPU_ONLINE_MASK 为 0，说明 boot_secondary_cpus 未被调用
        // 这是正常的，因为测试框架跳过了正常的启动流程
        if actual_mask == 0 {
            // 跳过此测试
            return;
        }

        let expected_mask = (1 << num_cpu) - 1;

        // 验证所有 CPU 都已上线
        kassert!(actual_mask == expected_mask);

        // 验证主核已上线
        kassert!((actual_mask & 1) != 0);

        // 如果是多核，验证从核也已上线
        if num_cpu > 1 {
            for hartid in 1..num_cpu {
                kassert!((actual_mask & (1 << hartid)) != 0);
            }
        }
    });
}

/// 从核入口函数
///
/// 由 SBI HSM 调用启动，hartid 通过 a0 寄存器传递。
///
/// Idle循环：等待中断；被 S 态时钟中断唤醒后，trap_handler 会决定是否调度
fn idle_loop() -> ! {
    loop {
        // 确保中断启用
        if !crate::arch::intr::are_interrupts_enabled() {
            unsafe {
                crate::arch::intr::enable_interrupts();
            }
        }

        // 等待中断（timer或IPI会触发调度）
        unsafe {
            core::arch::asm!("wfi");
        }
    }
}

/// 为指定CPU创建idle任务
fn create_idle_task(cpu_id: usize) -> crate::kernel::SharedTask {
    use crate::arch::trap::TrapFrame;
    use crate::ipc::{SignalHandlerTable, SignalPending};
    use crate::kernel::FsStruct;
    use crate::kernel::{TASK_MANAGER, TaskStruct};
    use crate::mm::frame_allocator::alloc_contig_frames;
    use crate::sync::SpinLock;
    use crate::uapi::resource::{INIT_RLIMITS, RlimitStruct};
    use crate::uapi::signal::SignalFlags;
    use crate::uapi::uts_namespace::UtsNamespace;
    use crate::vfs::fd_table::FDTable;
    use alloc::sync::Arc;
    use core::sync::atomic::Ordering;

    // idle任务从TID分配器正常分配TID
    // TID分配器从2开始，所以idle任务会获得TID 2, 3, ...
    // init进程会手动设置为TID 1
    let tid = TASK_MANAGER.lock().allocate_tid();

    // 分配最小资源
    let kstack_tracker =
        alloc_contig_frames(1).expect("Failed to allocate kernel stack for idle task");
    let trap_frame_tracker = alloc_frame().expect("Failed to allocate trap frame for idle task");

    // 创建最小化的任务结构
    let mut task = TaskStruct::ktask_create(
        tid,
        tid, // pid = tid
        0,   // ppid = 0 (no parent)
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

    // 设置trap frame指向idle_loop
    let tf = task.trap_frame_ptr.load(Ordering::SeqCst);
    unsafe {
        core::ptr::write(tf, TrapFrame::zero_init());
        (*tf).set_kernel_trap_frame(idle_loop as usize, 0, task.kstack_base);
    }

    // 设置CPU亲和性
    task.on_cpu = Some(cpu_id);

    let task = task.into_shared();

    // 将idle任务加入TaskManager
    // 现在idle任务使用正常的TID（2, 3, ...），不会冲突
    TASK_MANAGER.lock().add_task(task.clone());

    pr_info!("[SMP] Created idle task {} for CPU {}", tid, cpu_id);

    task
}

/// # 初始化流程
/// 1. 初始化 boot trap 处理（设置 stvec）
/// 2. 设置 tp 指向对应的 Cpu 结构体
/// 3. 标记 CPU 上线
/// 4. 禁用中断（避免 trap 处理问题）
/// 5. 进入 WFI 循环等待多核调度器实现
///
/// # 注意事项
/// - 从核使用 boot_trap_entry（不需要 sscratch）
/// - 从核禁用中断，避免在没有完整 trap 上下文时响应中断
/// - 等待多核调度器实现后，从核将被唤醒并启用中断
#[unsafe(no_mangle)]
pub extern "C" fn secondary_start(hartid: usize) -> ! {
    // 初始化 boot trap 处理，确保 stvec 指向 boot_trap_entry
    // boot_trap_entry 不需要 sscratch，使用栈保存上下文
    trap::init_boot_trap();

    // 设置 tp 指向对应的 Cpu 结构体
    {
        use crate::kernel::CPUS;
        let cpu_ptr = &*CPUS.get_of(hartid) as *const _ as usize;
        unsafe {
            core::arch::asm!("mv tp, {}", in(reg) cpu_ptr);
        }
    }

    // 标记当前 CPU 上线
    CPU_ONLINE_MASK.fetch_or(1 << hartid, Ordering::Release);

    pr_info!("[SMP] CPU {} is online", hartid);

    // 初始化完整的 trap 处理
    trap::init();

    // 创建并设置idle任务
    let idle_task = create_idle_task(hartid);

    // 设置sscratch指向idle任务的TrapFrame
    let tf_ptr = idle_task.lock().trap_frame_ptr.load(Ordering::SeqCst);
    unsafe {
        riscv::register::sscratch::write(tf_ptr as usize);
    }
    pr_debug!(
        "[SMP] CPU {} set sscratch to {:#x}",
        hartid,
        tf_ptr as usize
    );

    // 设置idle任务为当前任务，并记录为本CPU的idle句柄
    {
        let _guard = crate::sync::PreemptGuard::new();
        let cpu = current_cpu();
        cpu.idle_task = Some(idle_task.clone());
        cpu.switch_task(idle_task);
    }
    pr_info!("[SMP] CPU {} set idle task as current_task", hartid);

    // 切换到最终的内核页表（与 CPU0 共享），避免长期停留在 boot_pagetable
    if let Some(kernel_space) = crate::mm::get_global_kernel_space() {
        {
            let _guard = crate::sync::PreemptGuard::new();
            current_cpu().switch_space(kernel_space.clone());
        }
        let root_ppn = kernel_space.lock().root_ppn();
        pr_info!(
            "[SMP] CPU {} switched to global kernel space, root PPN: 0x{:x}",
            hartid,
            root_ppn.as_usize()
        );
    } else {
        pr_warn!(
            "[SMP] CPU {} could not get global kernel space; still on boot_pagetable",
            hartid
        );
    }

    // 初始化定时器
    timer::init();

    // 启用中断
    unsafe {
        intr::enable_interrupts();
    }

    // 检查中断配置状态
    unsafe {
        let sstatus: usize;
        let sie: usize;
        let sip: usize;
        core::arch::asm!("csrr {}, sstatus", out(reg) sstatus);
        core::arch::asm!("csrr {}, sie", out(reg) sie);
        core::arch::asm!("csrr {}, sip", out(reg) sip);
        pr_debug!(
            "[SMP] CPU {} interrupt status: sstatus={:#x}, sie={:#x}, sip={:#x}",
            hartid,
            sstatus,
            sie,
            sip
        );
        pr_debug!(
            "[SMP] CPU {} SIE bit: {}, SSIE bit: {}, SSIP bit: {}",
            hartid,
            (sstatus >> 1) & 1,
            (sie >> 1) & 1,
            (sip >> 1) & 1
        );
    }

    // 注意：mideleg 是 M-mode CSR，S-mode 无法读取
    // 如果尝试读取会触发非法指令异常
    // 我们需要通过其他方式验证中断委托配置

    pr_debug!("[SMP] CPU {} entering idle loop", hartid);

    // 进入idle循环（永不返回）
    idle_loop();
}

/// SBI HSM 从核入口（在 entry.S 中定义）
unsafe extern "C" {
    fn secondary_sbi_entry();
}

/// 启动从核（由主核调用）
///
/// # 参数
/// - num_cpus: 总 CPU 数量（包括主核）
///
/// # Panics
/// - 如果从核启动超时
pub fn boot_secondary_cpus(num_cpus: usize) {
    use crate::arch::timer::{clock_freq, get_time};

    if num_cpus <= 1 {
        pr_info!("[SMP] Single CPU mode, skipping secondary boot");
        // 标记主核在线
        CPU_ONLINE_MASK.fetch_or(1, Ordering::Release);
        unsafe { NUM_CPU = 1 };
        return;
    }

    pr_info!("[SMP] Booting up to {} secondary CPUs...", num_cpus - 1);

    // 主核标记在线
    CPU_ONLINE_MASK.fetch_or(1, Ordering::Release);

    // 尝试启动每个从核，记录预期应当在线的掩码（仅统计成功发起的启动请求）
    let mut expected_mask: usize = 1; // CPU0 已在线
    for hartid in 1..num_cpus {
        let start_vaddr = secondary_sbi_entry as usize;
        let start_paddr = unsafe { crate::arch::mm::vaddr_to_paddr(start_vaddr) };
        pr_info!(
            "[SMP] Starting hart {} at vaddr=0x{:x}, paddr=0x{:x}",
            hartid,
            start_vaddr,
            start_paddr
        );

        let ret = crate::arch::lib::sbi::hart_start(hartid, start_paddr, hartid);
        if ret.error != 0 {
            // HSM 不支持或被拒绝等，降级单核/少核而不是 panic
            pr_err!(
                "[SMP] Failed to start hart {}: SBI error {}",
                hartid,
                ret.error
            );
            continue;
        }
        expected_mask |= 1 << hartid;
        pr_info!("[SMP] Hart {} SBI call accepted", hartid);
    }

    // 若没有任何从核被接受启动请求，立即降级到单核
    if expected_mask == 1 {
        pr_warn!("[SMP] No secondary hart could be started; falling back to single-core");
        unsafe { NUM_CPU = 1 };
        return;
    }

    // 基于时间的超时等待（避免与主机性能相关的固定次数循环）
    // 设定 2 秒的上线等待窗口
    let deadline = get_time().saturating_add(clock_freq() * 2);
    while CPU_ONLINE_MASK.load(Ordering::Acquire) != expected_mask {
        if get_time() >= deadline {
            let current_mask = CPU_ONLINE_MASK.load(Ordering::Acquire);
            pr_warn!(
                "[SMP] Timeout waiting secondary CPUs. Expected: {:#b}, got: {:#b}",
                expected_mask,
                current_mask
            );
            break;
        }
        core::hint::spin_loop();
    }

    // 以实际在线核数为准，更新 NUM_CPU，避免后续调度把任务分配到离线 CPU
    let online_mask = CPU_ONLINE_MASK.load(Ordering::Acquire);
    let online_cnt = online_mask.count_ones() as usize;
    unsafe { NUM_CPU = core::cmp::max(online_cnt, 1) };

    if online_mask == expected_mask {
        pr_info!("[SMP] All {} CPUs are online!", unsafe { NUM_CPU });
    } else {
        pr_warn!(
            "[SMP] Proceeding with {} online CPU(s), mask={:#b}",
            unsafe { NUM_CPU },
            online_mask
        );
    }
}
