//! Futex 系统调用常量和标志定义。
//!
//! 这些常量对应于 Linux 内核的 futex(2) 系统调用操作码和控制标志。

use core::ffi::{c_long, c_void};

/// 类型定义：用于 Futex 系统调用操作码和标志。
pub type FutexOp = u32;

// --- Futex Operations (Basic) ---

/// 等待操作：如果 futex 地址处的值等于 val，则线程进入休眠。
pub const FUTEX_WAIT: FutexOp = 0;

/// 唤醒操作：唤醒至多 val 个等待在 futex 地址处的线程。
pub const FUTEX_WAKE: FutexOp = 1;

/// 文件描述符操作（历史遗留，很少直接使用）。
pub const FUTEX_FD: FutexOp = 2;

/// 重排队操作：唤醒 val 个线程，并将剩余线程从 uaddr 重新排队到 uaddr2。
pub const FUTEX_REQUEUE: FutexOp = 3;

/// 比较并重排队操作：类似于 REQUEUE，但在重排队前检查 uaddr 处的值是否等于 val2。
pub const FUTEX_CMP_REQUEUE: FutexOp = 4;

/// 唤醒并执行操作：执行一个原子操作并根据结果唤醒线程。用于实现信号量。
pub const FUTEX_WAKE_OP: FutexOp = 5;

// --- Priority Inheritance (PI) Futexes ---
// 用于实现优先级继承的互斥锁（Robust Mutexes）。

/// 锁定 PI 互斥体：尝试锁定 PI 互斥体。如果失败，等待并继承所有者的优先级。
pub const FUTEX_LOCK_PI: FutexOp = 6;

/// 解锁 PI 互斥体：释放 PI 互斥体。如果存在等待者，唤醒优先级最高的线程。
pub const FUTEX_UNLOCK_PI: FutexOp = 7;

/// 尝试锁定 PI 互斥体：非阻塞地尝试锁定 PI 互斥体。
pub const FUTEX_TRYLOCK_PI: FutexOp = 8;

/// 等待位集操作：等待，但只对 val3（位集）中包含的比特进行等待。用于高效的条件变量。
pub const FUTEX_WAIT_BITSET: FutexOp = 9;

// --- Futex Flags ---
// 这些标志通过位或操作（|）与操作码结合使用。

/// 私有标志：指定 futex 仅用于本进程内的线程同步。
/// （使用私有 futexs 通常更快，因为它不需要内核维护跨进程的哈希表）。
pub const FUTEX_PRIVATE: FutexOp = 128; // 0x80

/// 实时时钟标志：指定 FUTEX_WAIT 的超时时间应基于 CLOCK_REALTIME 计算。
/// （默认为 CLOCK_MONOTONIC）。
pub const FUTEX_CLOCK_REALTIME: FutexOp = 256; // 0x100

/// 健壮列表头部结构体（struct robust_list_head）
///
/// 这个结构体是用户空间维护的，用于告诉内核当前线程持有健壮 futex 锁的列表信息。
/// 示例用法（在需要时使用 volatile 访问）：
/// ```rust
/// fn access_robust_head(head: &mut RobustListHead) {
///    unsafe {
///        // 安全地读取 head 字段的 volatile 值
///        let first_lock_ptr = head.head.read_volatile();
///        // 安全地写入 head 字段的 volatile 值
///        head.head.write_volatile(core::ptr::null_mut());
///    }
/// }
/// ```
#[repr(C)]
pub struct RobustListHead {
    /// head: volatile void *volatile head;
    /// 指向当前线程拥有的第一个健壮 futex 锁。
    /// 在 C 中是 volatile void *，在 Rust 中使用 *mut c_void 或 *mut u8。
    /// 注意：volatile 访问必须通过 raw pointer 的 read/write_volatile 方法来实现，而不是在类型定义中。
    pub head: *mut c_void,

    /// off: long off;
    /// 列表项（futex 锁）中 owner_tid 字段相对于列表头部的字节偏移量。
    pub off: c_long,

    /// pending: volatile void *volatile pending;
    /// 指向一个正在等待被释放或修复的 futex 锁。
    pub pending: *mut c_void,
}
