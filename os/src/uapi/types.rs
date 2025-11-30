//! 基本类型定义，模拟 POSIX 标准中的类型
//!
//! 这些类型用于系统调用接口和内核与用户空间的交互。
//! 确保与 C 语言中的对应类型大小和对齐方式一致。

use core::ffi::{c_int, c_long, c_ulong};

use crate::uapi::signal::SignalStack;

/// 大小类型，通常用于表示对象的大小或内存块的大小。
pub type SizeT = c_ulong;
/// 进程 ID 类型
pub type PidT = c_int;
/// 用户 ID 类型
pub type UidT = c_int;
/// 时钟类型
pub type ClockT = c_int;
/// 长整型类型
pub type LongT = c_long;
/// 信号集合类型，表示一组信号的位掩码。
pub type SigSetT = u64;
/// 信号栈类型，用于描述备用信号处理栈的信息。
pub type StackT = SignalStack;
