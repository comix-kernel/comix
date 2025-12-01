//! Futex 系统调用常量和标志定义。
//!
//! 这些常量对应于 Linux 内核的 futex(2) 系统调用操作码和控制标志。

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
