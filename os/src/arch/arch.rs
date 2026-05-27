//! Arch trait — 顶层架构抽象
//!
//! 组合 `CpuOps + VirtualMemory`，并添加进程管理、信号处理、
//! 用户/内核内存复制等高层 CPU/MMU 操作。
//!
//! 平台级操作（控制台 I/O、电源管理、地址映射）已移至 [`crate::arch::platform::Platform`]。
//!
//! 注意：此 trait 使用关联类型来避免直接引用内核数据结构，
//! 确保架构层与内核其余部分的解耦。

use crate::arch::{address::UA, cpu_ops::CpuOps, virtual_memory::VirtualMemory};
use crate::mm::page_table::PagingError;

/// 顶层架构抽象 trait。
///
/// 组合了 `CpuOps` 和 `VirtualMemory`，并添加了进程管理、信号处理、
/// 用户/内核内存复制、时间、IPI 等架构级操作。
///
/// # 移植要点
///
/// 这是移植新架构时需要实现的第三个 trait（在 `CpuOps` 和 `VirtualMemory` 之后）。
/// [`Platform`] 应同时实现以覆盖控制台、电源等平台操作。
///
/// [`Platform`]: crate::arch::platform::Platform
pub trait Arch: CpuOps + VirtualMemory {
    /// 用户上下文类型（保存/恢复寄存器状态）
    type UserContext: Sized + Send + Sync + Clone;

    // ---- 进程 / 上下文切换 ----

    /// 创建新的用户上下文（设置入口点和栈顶）
    fn new_user_context(entry_point: usize, stack_top: usize) -> Self::UserContext;

    /// 上下文切换到指定用户上下文
    ///
    /// 保存当前执行上下文，恢复 `new_ctx` 的执行。
    ///
    /// # Safety
    ///
    /// 调用者必须确保 `new_ctx` 指向有效的用户上下文。
    unsafe fn context_switch(old: *mut Self::UserContext, new: *const Self::UserContext);

    // ---- 用户/内核内存复制 ----

    /// 从用户空间复制数据到内核空间
    ///
    /// # Safety
    ///
    /// - `src` 必须是有效的用户空间虚拟地址
    /// - `dst` 必须指向足够大的内核缓冲区
    /// - `len` 字节必须在合法范围内
    unsafe fn copy_from_user(src: UA, dst: *mut u8, len: usize) -> Result<(), PagingError>;

    /// 尝试从用户空间复制数据（非阻塞版本，不处理缺页）
    ///
    /// # Safety
    /// 同上
    unsafe fn try_copy_from_user(src: UA, dst: *mut u8, len: usize) -> Result<(), PagingError>;

    /// 从内核空间复制数据到用户空间
    ///
    /// # Safety
    ///
    /// - `dst` 必须是有效的用户空间虚拟地址
    /// - `src` 必须指向有效内核数据
    /// - `len` 字节必须在合法范围内
    unsafe fn copy_to_user(src: *const u8, dst: UA, len: usize) -> Result<(), PagingError>;

    /// 从用户空间复制以 '\0' 结尾的字符串
    ///
    /// # Safety
    /// 同上
    unsafe fn copy_strn_from_user(
        src: UA,
        dst: *mut u8,
        max_len: usize,
    ) -> Result<usize, PagingError>;

    // ---- 系统信息 ----

    /// 架构名称（如 "riscv64", "loongarch64"）
    fn name() -> &'static str;

    /// CPU 核心数量
    fn cpu_count() -> usize;

    // ---- 任务切换辅助 ----

    /// 任务切换时更新 trap frame 中的 CPU 指针
    ///
    /// 当任务在不同 CPU 之间迁移时，需要更新 trap frame 中的 `cpu_ptr` 字段，
    /// 确保 trap_entry 恢复正确的 tp 寄存器值。
    fn on_task_switch(trap_frame_ptr: usize, cpu_ptr: usize);

    // ---- 时间接口 ----

    /// 获取系统节拍计数
    fn get_ticks() -> usize;

    /// 获取系统启动以来的时间（节拍数）
    fn get_time() -> usize;

    /// 获取系统启动以来的时间（毫秒）
    fn get_time_ms() -> usize;

    /// 获取时钟频率（Hz）
    fn clock_freq() -> usize;

    // ---- IPI ----

    /// 向目标 CPU 发送重调度 IPI
    fn send_reschedule_ipi(target_cpu: usize);
}
