//! RISC-V 架构相关的启动代码

use core::{hint, sync::atomic::Ordering};

use riscv::register::sscratch;

use crate::{
    arch::{intr, mm::vaddr_to_paddr, timer, trap},
    kernel::{
        SCHEDULER, Scheduler, TASK_MANAGER, TaskManagerTrait, TaskStruct, current_cpu, into_shared,
        kernel_execve,
    },
    mm::{
        self,
        frame_allocator::{physical_page_alloc, physical_page_alloc_contiguous},
    },
    println,
    test::run_early_tests,
};
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

#[cfg(test)]
mod tests {

    use core::sync::atomic::Ordering;

    // 测试 create_kthreadd：应创建一个任务并加入 TASK_MANAGER
    use crate::{
        arch::boot::{create_kthreadd, kthreadd},
        kassert,
        kernel::{TASK_MANAGER, TaskManagerTrait},
        test_case,
    };

    test_case!(test_create_kthreadd, {
        // 记录当前已有任务数量
        let before_count = {
            let mgr = TASK_MANAGER.lock();
            mgr.task_count()
        };
        create_kthreadd();
        // 找到新增的任务（PID=tid，入口=kthreadd）
        let after_count = {
            let mgr = TASK_MANAGER.lock();
            mgr.task_count()
        };
        kassert!(after_count == before_count + 1);
        // 查找新 tid
        let new_tid = after_count as u32; // 简单假设 tid 连续分配
        let task = TASK_MANAGER.lock().get_task(new_tid).expect("task missing");
        let g = task.lock();
        let tf = g.trap_frame_ptr.load(Ordering::SeqCst);
        kassert!(g.tid == new_tid);
        kassert!(g.pid == new_tid); // kthreadd 设 pid=tid
        kassert!(unsafe { (*tf).sepc } as usize == kthreadd as usize);
    });

    // 由于 kernel_execve / rest_init / init / kthreadd 涉及不可返回的流控与实际陷入/页表切换，
    // 在单元测试环境下不执行它们（需要集成测试或仿真环境）。
}

pub fn main() {
    clear_bss();

    run_early_tests();

    // Initialize memory management (frame allocator + heap + kernel page table)
    mm::init();
    println!("Hello, world!");

    #[cfg(test)]
    crate::test_main();

    // 初始化工作
    trap::init_boot_trap();
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
