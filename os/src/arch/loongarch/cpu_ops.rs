//! LoongArch64 CpuOps 实现
//!
//! 提供 `LoongArch64` 结构体，实现 `arch::CpuOps` trait。
//! 将现有的 CSR 内联汇编操作映射到 trait 方法。

use crate::arch::CpuOps;
use crate::arch::constant::CSR_CRMD_IE;

/// LoongArch64 架构标记类型
pub struct LoongArch64;

impl CpuOps for LoongArch64 {
    /// 获取当前 CPU 核心 ID
    ///
    /// 从 tp 寄存器指向的 Cpu 结构体中读取首个字段 (cpu_id)。
    /// 如果 tp 为 0（尚未初始化），返回 0。
    #[inline]
    fn id() -> usize {
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
        unsafe { *(cpu_ptr as *const usize) }
    }

    /// 停止 CPU（通过 idle 循环等待）
    fn halt() -> ! {
        loop {
            unsafe {
                core::arch::asm!("idle 0", options(nomem, nostack));
            }
        }
    }

    /// 原子地禁用中断并返回之前的中断状态
    ///
    /// 通过修改 CRMD 寄存器的 IE 位来实现。
    #[inline]
    fn disable_interrupts() -> usize {
        let old: usize;
        unsafe {
            core::arch::asm!(
                "csrrd {old}, 0x0",
                old = out(reg) old,
                options(nostack, preserves_flags)
            );
        }
        let new = old & !CSR_CRMD_IE;
        unsafe {
            core::arch::asm!(
                "csrwr {new}, 0x0",
                new = in(reg) new,
                options(nostack, preserves_flags)
            );
        }
        old
    }

    /// 恢复之前保存的中断状态
    ///
    /// 根据保存的 CRMD 值恢复 IE 位。
    #[inline]
    fn restore_interrupt_state(flags: usize) {
        if flags & CSR_CRMD_IE != 0 {
            Self::enable_interrupts();
        }
    }

    /// 显式启用中断
    #[inline]
    fn enable_interrupts() {
        let old: usize;
        unsafe {
            core::arch::asm!(
                "csrrd {old}, 0x0",
                old = out(reg) old,
                options(nostack, preserves_flags)
            );
        }
        let new = old | CSR_CRMD_IE;
        unsafe {
            core::arch::asm!(
                "csrwr {new}, 0x0",
                new = in(reg) new,
                options(nostack, preserves_flags)
            );
        }
    }

    #[inline]
    fn interrupts_enabled() -> bool {
        let crmd: usize;
        unsafe {
            core::arch::asm!(
                "csrrd {crmd}, 0x0",
                crmd = out(reg) crmd,
                options(nostack, preserves_flags)
            );
        }
        crmd & CSR_CRMD_IE != 0
    }

    #[inline]
    fn interrupt_was_enabled(flags: usize) -> bool {
        flags & CSR_CRMD_IE != 0
    }
}
