//! RISC-V 架构的中断管理
#![allow(unused)]

use crate::sync::SpinLock;

/// 软中断枚举
pub enum Softirq {
    HiSoftirq,
    TimerSoftirq,
    NetTxSoftirq,
    NetRxSoftirq,
    BlockSoftirq,
    IrqPollSoftirq,
    TaskletSoftirq,
    SchedSoftirq,
    HrtimerSoftirq,
    RcuSoftirq, /* Preferable RCU should always be the last softirq */
    NrSoftirqs,
}

/// 触发软中断
/// 参数:
/// * `softirq` - 要触发的软中断类型
/// 安全性: 该函数涉及底层中断处理机制，可能会引发竞态条件或系统不稳定。
/// 调用者必须确保在适当的上下文中调用此函数，以避免潜在的问题。
#[unsafe(no_mangle)]
pub fn raise_softirq(softirq: Softirq) {
    unimplemented!("raise_softirq is not implemented yet");
}
