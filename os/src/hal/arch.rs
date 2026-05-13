//! Arch trait — 顶层架构抽象
//!
//! 组合 `CpuOps + VirtualMemory`，并添加进程管理、信号处理、
//! 用户/内核内存复制、系统信息、电源管理等高层操作。
//!
//! 注意：此 trait 使用关联类型来避免直接引用内核数据结构，
//! 确保 HAL 层与内核其余部分的解耦。

use crate::hal::cpu_ops::CpuOps;
use crate::hal::virtual_memory::VirtualMemory;
/// 顶层架构抽象 trait。
///
/// 组合了 `CpuOps` 和 `VirtualMemory`，并添加了进程管理、信号处理、
/// 用户/内核内存复制等高层架构特定操作。
///
/// # 移植要点
///
/// 这是移植新架构时需要实现的第三个 trait（在 `CpuOps` 和 `VirtualMemory` 之后）。
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
    unsafe fn copy_from_user(src: usize, dst: *mut u8, len: usize) -> Result<(), ()>;

    /// 尝试从用户空间复制数据（非阻塞版本，不处理缺页）
    ///
    /// # Safety
    /// 同上
    unsafe fn try_copy_from_user(src: usize, dst: *mut u8, len: usize) -> Result<(), ()>;

    /// 从内核空间复制数据到用户空间
    ///
    /// # Safety
    ///
    /// - `dst` 必须是有效的用户空间虚拟地址
    /// - `src` 必须指向有效内核数据
    /// - `len` 字节必须在合法范围内
    unsafe fn copy_to_user(src: *const u8, dst: usize, len: usize) -> Result<(), ()>;

    /// 从用户空间复制以 '\0' 结尾的字符串
    ///
    /// # Safety
    /// 同上
    unsafe fn copy_strn_from_user(src: usize, dst: *mut u8, max_len: usize) -> Result<usize, ()>;

    // ---- 系统信息 ----

    /// 架构名称（如 "riscv64", "loongarch64"）
    fn name() -> &'static str;

    /// CPU 核心数量
    fn cpu_count() -> usize;

    /// 获取内核命令行参数
    fn get_cmdline() -> Option<alloc::string::String>;

    // ---- 电源管理 ----

    /// 关机，永不返回
    fn power_off() -> !;

    /// 重启，永不返回
    fn restart() -> !;
}
