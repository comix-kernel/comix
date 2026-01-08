//! LoongArch64 内核任务模块

use core::arch::global_asm;

pub mod context;
pub mod task;

global_asm!(include_str!("switch.S"));

// 上下文切换函数
unsafe extern "C" {
    pub fn switch(old: *mut Context, new: *const Context);
}

/// CPU 相关
pub mod cpu {
    /// 获取当前 Hart ID（当前仅单核）
    pub fn hart_id() -> usize {
        0
    }

    /// 获取 CPU ID（别名）
    pub fn cpu_id() -> usize {
        let cpu_ptr: usize;
        unsafe {
            core::arch::asm!(
                "addi.d {0}, $tp, 0",
                out(reg) cpu_ptr,
                options(nostack, preserves_flags)
            );
        }
        if cpu_ptr == 0 {
            return 0;
        }
        // Safety: tp 指向 Cpu 结构体，首字段为 cpu_id
        unsafe { *(cpu_ptr as *const usize) }
    }

    /// 在切换到指定任务后执行的架构相关收尾工作。
    ///
    /// LoongArch 使用 KScratch0 保存当前任务的 TrapFrame 指针，
    /// 供 trap_entry 保存/恢复寄存器时使用。
    pub fn on_task_switch(trap_frame_ptr: usize, cpu_ptr: usize) {
        if trap_frame_ptr == 0 {
            return;
        }
        unsafe {
            let tf = (trap_frame_ptr as *mut crate::arch::trap::TrapFrame)
                .as_mut()
                .expect("on_task_switch: null TrapFrame");
            tf.cpu_ptr = cpu_ptr;
            core::arch::asm!(
                "addi.d $tp, {0}, 0",
                in(reg) cpu_ptr,
                options(nostack, preserves_flags)
            );
            // KScratch0 作为 trap_entry 的 TrapFrame 指针。
            core::arch::asm!(
                "csrwr {0}, 0x48",
                in(reg) trap_frame_ptr,
                options(nostack, preserves_flags)
            );
        }
    }
}

pub use context::TaskContext;

/// Context 类型别名（用于兼容）
pub type Context = TaskContext;
