//! 内核任务相关功能实现
//!
//! 包括内核线程创建、等待、执行用户程序等功能
//! 内核任务不具备用户态任务的内存空间和权限
//! 仅在内核态运行
use core::{hint, sync::atomic::Ordering};

use alloc::string::ToString;
use alloc::sync::Arc;

use crate::{
    arch::{intr::disable_interrupts, trap::restore},
    kernel::{
        TaskState,
        cpu::current_cpu,
        scheduler::Scheduler,
        task::{TASK_MANAGER, TaskStruct, task_manager::TaskManagerTrait},
    },
    mm::frame_allocator::{alloc_contig_frames, alloc_frame},
    sync::SpinLock,
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
    let tid = TASK_MANAGER.lock().allocate_tid();
    let (pid, ppid, signal_handlers, blocked, signal, uts, rlimit, fd_table, fs) = {
        let _guard = crate::sync::PreemptGuard::new();
        let cur_cpu = current_cpu();
        let cur_task = cur_cpu.current_task.as_ref().unwrap();
        let cur_task = cur_task.lock();
        (
            cur_task.pid,
            cur_task.ppid,
            cur_task.signal_handlers.clone(),
            cur_task.blocked,
            cur_task.shared_pending.clone(),
            cur_task.uts_namespace.clone(),
            cur_task.rlimit.clone(),
            cur_task.fd_table.clone(),
            cur_task.fs.clone(),
        )
    };

    let kstack_tracker = alloc_contig_frames(4).expect("kthread_spawn: failed to alloc kstack");
    let trap_frame_tracker = alloc_frame().expect("kthread_spawn: failed to alloc trap_frame");

    // 分配 Task 结构体和内核栈
    let task = TaskStruct::ktask_create(
        tid,
        pid,
        ppid,
        TaskStruct::empty_children(),
        kstack_tracker,
        trap_frame_tracker,
        signal_handlers,
        blocked,
        signal,
        uts,
        rlimit,
        fd_table,
        fs,
    );

    let tf = task.trap_frame_ptr.load(Ordering::SeqCst);
    // SAFETY: 此时 trap_frame_tracker 已经分配完毕且不可变更，所有权在 task 中，指针有效
    unsafe {
        // 先初始化 TrapFrame 为全 0
        core::ptr::write(tf, crate::arch::trap::TrapFrame::zero_init());
        (*tf).set_kernel_trap_frame(
            entry_point as usize,
            super::terminate_task as usize,
            task.kstack_base,
        );
        let cpu_ptr = {
            let _guard = crate::sync::PreemptGuard::new();
            crate::kernel::current_cpu() as *const _ as usize
        };
        crate::arch::trap::set_trap_frame_cpu_ptr(tf, cpu_ptr);
    }
    let tid = task.tid;
    let task = task.into_shared();

    // 选择目标 CPU（负载均衡）
    let target_cpu = crate::kernel::pick_cpu();

    // 更新任务的 on_cpu 字段
    task.lock().on_cpu = Some(target_cpu);

    crate::pr_debug!("[SMP] Task {} assigned to CPU {}", tid, target_cpu);

    // 将任务加入调度器和任务管理器
    TASK_MANAGER.lock().add_task(task.clone());
    crate::kernel::scheduler_of(target_cpu)
        .lock()
        .add_task(task);

    // 如果目标 CPU 不是当前 CPU，发送 IPI
    let current_cpu = crate::arch::kernel::cpu::cpu_id();
    if target_cpu != current_cpu {
        crate::arch::ipi::send_reschedule_ipi(target_cpu);
    }

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
/// # 安全性
/// 调用者必须保证 `return_value_ptr` 指向的内存是合法可写的
pub unsafe fn kthread_join(tid: u32, return_value_ptr: Option<usize>) -> i32 {
    loop {
        let task_opt = TASK_MANAGER.lock().get_task(tid);
        if let Some(task) = task_opt {
            let t = task.lock();
            if t.state == TaskState::Zombie {
                if let Some(rv) = t.exit_code {
                    // SAFETY: 调用者保证了 return_value_ptr 指向的内存是合法可写的
                    unsafe {
                        if let Some(ptr) = return_value_ptr {
                            let ptr = ptr as *mut usize;
                            ptr.write_volatile(rv as usize);
                        }
                    }
                }
                TASK_MANAGER.lock().release_task(task.clone());
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
    // 1. 加载并准备可执行映像（支持 PT_INTERP）
    crate::pr_info!("[kernel_execve] Loading: {}", path);
    let prepared =
        super::prepare_exec_image_from_path(path).expect("kernel_execve: failed to prepare image");
    crate::pr_info!(
        "[kernel_execve] Prepared image, pc=0x{:x}, at_base=0x{:x}, at_entry=0x{:x}",
        prepared.initial_pc,
        prepared.at_base,
        prepared.at_entry
    );

    // 2. 包装内存空间
    let space = Arc::new(SpinLock::new(prepared.space));
    // 换掉当前任务的地址空间，e.g. 切换 satp
    {
        let _guard = crate::sync::PreemptGuard::new();
        current_cpu().switch_space(space.clone());
    }

    let task = {
        let _guard = crate::sync::PreemptGuard::new();
        let cpu = current_cpu();
        cpu.current_task.as_ref().unwrap().clone()
    };
    // 在restore之前不可发生中断
    // execve伪造进程上下文用的trapframe和当前进程的是同一个
    // 这时候发生中断会破坏创建到一半/创建好的的上下文
    // 不必显式恢复中断，它会在restore中由sret指令自动恢复
    unsafe { disable_interrupts() };
    {
        let mut t = task.lock();
        t.exe_path = Some(path.to_string());
        t.execve(
            space,
            prepared.initial_pc,
            prepared.user_sp_high,
            argv,
            envp,
            prepared.phdr_addr,
            prepared.phnum,
            prepared.phent,
            prepared.at_base,
            prepared.at_entry,
        );
    }
    crate::pr_info!("[kernel_execve] Switching to user mode");

    let tfp = task.lock().trap_frame_ptr.load(Ordering::SeqCst);
    // SAFETY: tfp 指向的内存已经被分配且由当前任务拥有
    // 直接按 trapframe 状态恢复并 sret 到用户态
    unsafe {
        restore(&*tfp);
    }
    unreachable!("kernel_execve: should not return");
}

#[cfg(test)]
mod tests {
    // TODO: kthread_spawn 内部依赖全局状态CPU, 现在无法进行测试
    #![allow(dead_code)]
    // use alloc::vec::Vec;

    use super::*;
    use crate::{
        kassert,
        kernel::task::{SharedTask, TASK_MANAGER},
        test_case,
    };
    // use core::sync::atomic::Ordering;

    // 创建一个简单的空函数作为 kernel 线程入口
    fn dummy_thread() {}

    fn mk_task(tid: u32) -> SharedTask {
        TaskStruct::new_dummy_task(tid).into_shared()
    }

    // 测试 kthread_spawn：应分配 tid 并放入任务管理器
    test_case!(test_kthread_spawn_basic, {
        {
            let _guard = crate::sync::PreemptGuard::new();
            current_cpu().current_task = Some(mk_task(1));
        }
        let tid = kthread_spawn(dummy_thread);
        kassert!(tid != 0);
        let task_opt = TASK_MANAGER.lock().get_task(tid);
        kassert!(task_opt.is_some());
        let t = task_opt.unwrap();
        let g = t.lock();
        kassert!(g.tid == tid);
        kassert!(g.is_kernel_thread());
    });

    // 测试 kthread_join 成功路径：预置一个 Stopped 状态的任务与返回值
    // test_case!(test_kthread_join_success, {
    //     // 预创建任务
    //     let tid = TASK_MANAGER.lock().allocate_tid();
    //     let kstack_tracker =
    //         crate::mm::frame_allocator::physical_page_alloc_contiguous(1).expect("alloc kstack");
    //     let trap_frame_tracker =
    //         crate::mm::frame_allocator::physical_page_alloc().expect("alloc trap_frame");
    //     let task = TaskStruct::ktask_create(
    //         tid,
    //         tid,
    //         0,
    //         kstack_tracker,
    //         trap_frame_tracker,
    //         dummy_thread as usize,
    //     );
    //     let shared = into_shared(task);
    //     {
    //         let mut g = shared.lock();
    //         g.state = TaskState::Stopped;
    //         g.return_value = Some(0xDEAD_BEEF);
    //     }
    //     TASK_MANAGER.lock().add_task(shared.clone());
    //     SCHEDULER.lock().add_task(shared);

    //     // 为返回值提供缓冲区
    //     let mut rv_slot: usize = 0;
    //     let rc = kthread_join(tid, Some(&mut rv_slot as *mut usize as usize));
    //     kassert!(rc == 0);
    //     kassert!(rv_slot == 0xDEAD_BEEF);
    //     // 任务应已从管理器移除
    //     kassert!(TASK_MANAGER.lock().get_task(tid).is_none());
    // });

    // 测试 kthread_join 失败路径：不存在的 tid
    test_case!(test_kthread_join_not_found, {
        // 选择一个极小概率已存在的高 tid（或先确保不存在）
        let missing_tid = 0xFFFF_FFFFu32;
        kassert!(TASK_MANAGER.lock().get_task(missing_tid).is_none());
        let rc = unsafe { kthread_join(missing_tid, None) };
        kassert!(rc == -1);
    });
}
