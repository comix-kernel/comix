//! RISC-V CpuOps 实现
//!
//! 提供 `Riscv64` 结构体，实现 `hal::CpuOps` trait。
//! 将现有的 CSR 内联汇编操作映射到 trait 方法。

use crate::hal::CpuOps;
use riscv::register::sstatus;

/// RISC-V 64 架构标记类型
pub struct Riscv64;

impl CpuOps for Riscv64 {
    /// 获取当前 CPU 核心 ID
    ///
    /// 从 tp 寄存器指向的 Cpu 结构体中读取首个字段 (cpu_id)。
    /// 在内核态，tp 指向 Cpu 结构体。
    #[inline]
    fn id() -> usize {
        let id: usize;
        unsafe {
            core::arch::asm!(
                "ld {}, 0(tp)",
                out(reg) id
            );
        }
        id
    }

    /// 停止 CPU（通过 WFI 指令循环等待）
    fn halt() -> ! {
        loop {
            unsafe {
                core::arch::asm!("wfi", options(nomem, nostack));
            }
        }
    }

    /// 原子地禁用中断并返回之前的中断状态
    ///
    /// 通过 CSRRC 指令原子地清除 sstatus.SIE 位。
    #[inline]
    fn disable_interrupts() -> usize {
        let old: usize;
        // SIE 在 sstatus 的位 1
        let sie_mask: usize = 1 << 1;
        unsafe {
            core::arch::asm!(
                "csrrc {old}, sstatus, {mask}",
                old = out(reg) old,
                mask = in(reg) sie_mask,
                options(nomem, nostack)
            );
        }
        old
    }

    /// 恢复之前保存的中断状态
    ///
    /// 如果保存的状态中 SIE 位为 1，则重新启用中断。
    #[inline]
    fn restore_interrupt_state(flags: usize) {
        // SSTATUS_SIE 定义在 arch::constant
        let spie: usize = flags & crate::arch::constant::SSTATUS_SIE;
        if spie != 0 {
            unsafe { sstatus::set_sie() };
        }
    }

    /// 显式启用中断
    #[inline]
    fn enable_interrupts() {
        unsafe { sstatus::set_sie() };
    }

    #[inline]
    fn interrupt_was_enabled(flags: usize) -> bool {
        flags & crate::arch::constant::SSTATUS_SIE != 0
    }
}
