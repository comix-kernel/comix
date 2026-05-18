//! CpuOps — 最底层架构抽象 trait
//!
//! 将架构相关操作缩小到最少 6 个方法，使得 sync/memory 等模块完全可移植。

/// CPU 操作抽象 trait。
///
/// 所有对 CPU 状态的操作（中断开关、核心 ID、停机）都通过此 trait 进行，
/// 使得同步原语等可以泛型化并可在宿主上 Mock 测试。
///
/// # 移植要点
///
/// 这是移植新架构时第一个需要实现的 trait。只需 6 个方法，实现后即可编译
/// 同步原语和内存分配器等核心模块。
pub trait CpuOps: 'static {
    /// 获取当前 CPU 核心 ID
    fn id() -> usize;

    /// 停止 CPU，永不返回
    fn halt() -> !;

    /// 禁用中断并返回之前的中断状态
    ///
    /// 返回的 `usize` 值可用于 `restore_interrupt_state` 恢复之前的状态。
    fn disable_interrupts() -> usize;

    /// 恢复之前保存的中断状态
    ///
    /// # Safety
    ///
    /// 调用者必须确保 `flags` 来自 `disable_interrupts` 或 `read_and_enable_interrupts` 的返回值。
    fn restore_interrupt_state(flags: usize);

    /// 显式启用中断
    fn enable_interrupts();

    /// 当前中断是否处于启用状态
    fn interrupts_enabled() -> bool;

    /// 检查 `disable_interrupts()` 返回的 flags 中中断是否处于启用状态
    ///
    /// 默认实现假定 flags 的 bit 0 表示中断启用状态。
    fn interrupt_was_enabled(flags: usize) -> bool {
        flags & 1 != 0
    }
}
