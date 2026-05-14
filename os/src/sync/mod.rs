//! 同步原语
//!
//! 向其它内核模块提供基本的锁和同步原语
//! 包括自旋锁、睡眠锁、中断保护等
mod intr_guard;
#[cfg(feature = "proc")]
mod mutex;
mod per_cpu;
mod preempt;
mod raw_spin_lock;
mod raw_spin_lock_without_guard;
mod rwlock;
mod spin_lock;
mod ticket_lock;

#[cfg(feature = "proc")]
pub use mutex::*;
pub use per_cpu::PerCpu;
pub use preempt::PreemptGuard;
pub use raw_spin_lock::*;
pub use raw_spin_lock_without_guard::*;
pub use rwlock::*;
pub use spin_lock::*;
