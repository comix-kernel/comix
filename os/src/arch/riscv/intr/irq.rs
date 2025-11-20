//! RISC-V 架构的中断请求 (IRQ) 模块
//!
//! 该模块提供了 RISC-V 架构下中断请求的管理功能，包括注册和注销中断处理程序等。
//! 中断处理程序可以通过 `request_irq` 函数注册，并通过 `free_irq` 函数注销。
#![allow(unused)]

use crate::sync::SpinLock;

lazy_static::lazy_static! {
    /// 全局中断管理器实例
    pub static ref IRQ_MANAGER: SpinLock<IrqManager> = SpinLock::new(IrqManager::new());
}

/// 中断管理器结构体
/// 该结构体包含一个中断处理函数的映射表
/// 每个中断号对应一个可选的处理函数
/// TODO: 支持更多中断管理功能，如中断优先级、中断屏蔽等
pub struct IrqManager {
    // 中断处理函数映射表
    handlers: [Option<fn()>; 256],
}

impl IrqManager {
    /// 创建一个新的中断管理器实例
    pub fn new() -> Self {
        IrqManager {
            handlers: [None; 256],
        }
    }

    pub fn get_handler(&self, irq_number: usize) -> Option<fn()> {
        self.handlers[irq_number]
    }
}

/// 注册中断处理程序
/// 参数:
/// * `irq_number`: 中断号
/// * `handler`: 中断处理函数
/// 返回值: 如果注册成功，返回 true；否则返回 false
#[unsafe(no_mangle)]
pub fn request_irq(irq_number: usize, handler: fn()) -> bool {
    IRQ_MANAGER.lock().handlers[irq_number] = Some(handler);
    true
}

/// 注销中断处理程序
/// 参数:
/// * `irq_number`: 中断号
#[unsafe(no_mangle)]
pub fn free_irq(irq_number: usize) {
    IRQ_MANAGER.lock().handlers[irq_number] = None;
}

/// 软件中断枚举
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

/// 触发软件中断
/// 参数:
/// * `irq_number`: 中断号
#[unsafe(no_mangle)]
pub fn raise_softirq(softirq: Softirq) {
    unimplemented!("raise_softirq is not implemented yet");
}
