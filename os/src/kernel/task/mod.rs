use core::{hint, sync::atomic::Ordering};

use alloc::sync::Arc;
use lazy_static::lazy_static;

mod task_manager;
mod task_state;
mod task_struct;
mod tid_allocator;

use riscv::register::sscratch;
pub use task_state::TaskState;
pub use task_struct::Task as TaskStruct;

pub type SharedTask = Arc<SpinLock<TaskStruct>>;

use crate::{
    arch::trap::{self, TrapFrame, restore},
    kernel::{
        cpu::current_cpu,
        scheduler::{SCHEDULER, Scheduler},
        task::task_manager::TaskManager,
    },
    mm::frame_allocator::{physical_page_alloc, physical_page_alloc_contiguous},
    sync::spin_lock::SpinLock,
};

lazy_static! {
    static ref TASK_MANAGER: SpinLock<TaskManager> = SpinLock::new(TaskManager::new());
}

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
    let ppid = {
        let cur_cpu = current_cpu().lock();
        let cur_task = cur_cpu.current_task.as_ref().unwrap();
        cur_task.lock().pid
    };
    let kstack_tracker =
        physical_page_alloc_contiguous(4).expect("kthread_spawn: failed to alloc kstack");
    let trap_frame_tracker =
        physical_page_alloc().expect("kthread_spawn: failed to alloc trap_frame");

    // 分配 Task 结构体和内核栈
    let task = TaskStruct::ktask_create(tid, ppid, kstack_tracker, trap_frame_tracker, entry_addr);
    let tid = task.tid;
    let task = into_shared(task);

    // 将任务加入调度器和任务管理器
    TASK_MANAGER.lock().add_task(task.clone());
    // 将任务加入全局任务队列
    SCHEDULER.lock().add_task(task);

    tid
}

/// 把已初始化的 TaskStruct 包装为共享任务句柄
pub fn into_shared(task: TaskStruct) -> SharedTask {
    Arc::new(SpinLock::new(task))
}

fn a() {
    loop {
        for _ in 0..1000 {
            core::hint::spin_loop();
        }
        print!("A");
    }
}

fn b() {
    loop {
        for _ in 0..1000 {
            core::hint::spin_loop();
        }
        print!("B");
    }
}

/// 内核初始化后将第一个任务(kinit)放到 CPU 上运行
/// 并且当这个函数结束时，应该切换到第一个任务的上下文
pub fn kinit_entry() {
    let tid = TASK_MANAGER.lock().allocate_tid();
    let kstack_tracker =
        physical_page_alloc_contiguous(4).expect("kthread_spawn: failed to alloc kstack");
    let trap_frame_tracker =
        physical_page_alloc().expect("kthread_spawn: failed to alloc trap_frame");
    let task = into_shared(TaskStruct::ktask_create(
        tid,
        0,
        kstack_tracker,
        trap_frame_tracker,
        kinit as usize,
    )); // kinit 没有父任务

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
/// 在初始化完成后由调度器运行
/// TODO: 现在只是一个空循环
fn kinit() {
    trap::init();
    kthread_spawn(a);
    kthread_spawn(b);
    // unsafe { intr::enable_interrupts() };
    loop {
        for _ in 0..1000 {
            core::hint::spin_loop();
        }
        print!("C");
        hint::spin_loop();
    }
}

/// 新创建的线程发生第一次调度时会从 forkret 开始执行
#[allow(dead_code)]
pub fn forkret() {
    let fp: *mut TrapFrame;
    {
        let cpu = current_cpu().lock();
        let task = cpu.current_task.as_ref().unwrap();
        fp = task.lock().trap_frame_ptr.load(Ordering::SeqCst);
    }
    unsafe { restore(&*fp) };
}
