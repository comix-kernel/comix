//! 体系结构相关的模块
//!
//! 包含与特定处理器架构相关的实现。
//! 根据目标架构选择性地包含不同的子模块。
//!
//! # 分层约定
//!
//! 为了减少在 `arch/` 之外散落的 `cfg(target_arch = ...)` 与架构特定依赖：
//! - **架构条件编译应尽量集中在本模块**（选择 `riscv/` 或 `loongarch/`）。
//! - `arch/` 外部代码应通过 `crate::arch::*` 暴露的统一接口/钩子访问架构差异，
//!   避免直接依赖 `riscv`、`loongArch64` 等架构专用 crate 或寄存器操作。

// ---- trait 定义 ----

pub mod abi;
pub mod address;
pub mod arch;
pub mod cpu_ops;
pub mod plat;
pub mod task;
pub mod virtual_memory;

pub use arch::Arch;
pub use cpu_ops::CpuOps;
pub use plat::Platform;

// ---- 共享模块（架构无关） ----

mod arch_impl;
mod memory_impl;

// ---- 目标架构：RISC-V / LoongArch ----

#[cfg(target_arch = "loongarch64")]
mod loongarch;

#[cfg(target_arch = "riscv64")]
mod riscv;

#[cfg(target_arch = "loongarch64")]
pub use loongarch::*;

#[cfg(target_arch = "riscv64")]
pub use riscv::*;

// ---- 非目标架构（宿主测试）：Mock Stubs ----

#[cfg(not(any(target_arch = "riscv64", target_arch = "loongarch64")))]
mod mock;

#[cfg(not(any(target_arch = "riscv64", target_arch = "loongarch64")))]
pub use mock::*;

pub use constant::SUPERVISOR_EXTERNAL;

// ---- ArchImpl 类型别名 ----
// 内核其余部分通过 ArchImpl 访问架构特定功能，无需关心具体架构。
#[cfg(target_arch = "riscv64")]
pub use riscv::cpu_ops::Riscv64 as ArchImpl;

#[cfg(target_arch = "loongarch64")]
pub use loongarch::cpu_ops::LoongArch64 as ArchImpl;

// 宿主（非目标架构）使用 Mock 实现
#[cfg(not(any(target_arch = "riscv64", target_arch = "loongarch64")))]
pub use crate::arch::mock::MockArch as ArchImpl;

// ---- PlatformImpl 类型别名 ----
// 与 ArchImpl 对应，内核其余部分通过 PlatformImpl 访问平台功能。

#[cfg(target_arch = "riscv64")]
pub use riscv::cpu_ops::Riscv64 as PlatformImpl;

#[cfg(target_arch = "loongarch64")]
pub use loongarch::cpu_ops::LoongArch64 as PlatformImpl;

#[cfg(not(any(target_arch = "riscv64", target_arch = "loongarch64")))]
pub use crate::arch::mock::MockArch as PlatformImpl;

// ---- 便捷包装函数 ----
// 将 Arch / Platform trait 方法暴露为普通函数。

/// 启用中断
#[inline]
pub fn enable_interrupts() {
    ArchImpl::enable_interrupts()
}

/// 当前中断是否处于启用状态
#[inline]
pub fn interrupts_enabled() -> bool {
    ArchImpl::interrupts_enabled()
}

/// 禁用中断并返回之前的中断状态
#[inline]
pub fn disable_interrupts() -> usize {
    ArchImpl::disable_interrupts()
}

/// 恢复中断状态
#[inline]
pub fn restore_interrupt_state(flags: usize) {
    ArchImpl::restore_interrupt_state(flags)
}

/// 获取当前 CPU ID
#[inline]
pub fn cpu_id() -> usize {
    ArchImpl::id()
}

/// 任务切换时更新 trap frame CPU 指针
#[inline]
pub fn on_task_switch(trap_frame_ptr: usize, cpu_ptr: usize) {
    ArchImpl::on_task_switch(trap_frame_ptr, cpu_ptr)
}

/// 获取系统节拍
#[inline]
pub fn get_ticks() -> usize {
    ArchImpl::get_ticks()
}

/// 获取系统时间（节拍）
#[inline]
pub fn get_time() -> usize {
    ArchImpl::get_time()
}

/// 获取系统时间（毫秒）
#[inline]
pub fn get_time_ms() -> usize {
    ArchImpl::get_time_ms()
}

/// 获取时钟频率
#[inline]
pub fn clock_freq() -> usize {
    ArchImpl::clock_freq()
}

/// 发送重调度 IPI
#[inline]
pub fn send_reschedule_ipi(target: usize) {
    ArchImpl::send_reschedule_ipi(target)
}

/// 物理地址 → 虚拟地址（直接映射）
#[inline]
pub fn pa_to_va(pa: address::PA) -> address::VA {
    PlatformImpl::pa_to_va(pa)
}

/// 虚拟地址 → 物理地址（直接映射）
///
/// # Safety
/// 调用者需确保 `va` 处于直接映射区域。
#[inline]
pub unsafe fn va_to_pa(va: address::VA) -> address::PA {
    unsafe { PlatformImpl::va_to_pa(va) }
}

/// 判断虚拟地址是否位于直接映射区域。
#[inline]
pub fn is_direct_mapped_va(va: address::VA) -> bool {
    va.as_usize() >= <PlatformImpl as virtual_memory::VirtualMemory>::PAGE_OFFSET
}
