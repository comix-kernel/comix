//! Platform trait — 平台抽象层
//!
//! 从 `Arch` trait 剥离的平台级操作（控制台 I/O、电源管理、
//! 地址映射策略、启动参数）。此 trait 关注的是 *板级/机器级*
//! 差异，而非 CPU 架构差异。
//!
//! 默认物理→虚拟地址映射使用 `PAGE_OFFSET` 偏移，架构可覆写。

use crate::arch::virtual_memory::VirtualMemory;

/// 平台抽象 trait。
///
/// 负责控制台 I/O、电源管理、地址映射等平台级操作。
///
/// # 移植要点
///
/// 实现 `CpuOps` + `VirtualMemory` + `Arch` 后实现本 trait。
/// 多数方法有基于 `PAGE_OFFSET` 的默认实现。
pub trait Platform: VirtualMemory {
    /// 向调试控制台输出一个字节
    fn console_putchar(c: u8);

    /// 从调试控制台读取一个字节（非阻塞），`None` 表示无输入
    fn console_getchar() -> Option<u8>;

    /// 获取内核命令行参数
    fn get_cmdline() -> Option<alloc::string::String>;

    /// 物理地址 → 虚拟地址（直接映射区域）
    fn paddr_to_vaddr(paddr: usize) -> usize {
        paddr + Self::PAGE_OFFSET
    }

    /// 虚拟地址 → 物理地址（直接映射区域）
    ///
    /// # Safety
    /// 调用者需确保 `vaddr` 处于直接映射范围内。
    unsafe fn vaddr_to_paddr(vaddr: usize) -> usize {
        vaddr - Self::PAGE_OFFSET
    }

    /// 关机，永不返回
    fn power_off() -> !;

    /// 重启，永不返回
    fn restart() -> !;
}
