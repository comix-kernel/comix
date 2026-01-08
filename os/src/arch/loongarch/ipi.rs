//! LoongArch64 IPI（单核下的最小实现）
//!
//! 目前 LoongArch 端尚未接入多核 IPI 硬件。
//! 单核环境下 IPI 调用不需要实际发起中断，保留接口以复用通用调度/任务迁移逻辑。

/// IPI 类型（用于与 RISC-V 端保持接口一致）
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IpiType {
    /// 通知目标 CPU 进行 reschedule
    Reschedule,
    /// 通知目标 CPU 刷新 TLB
    TlbFlush,
}

/// 发送单个 IPI（单核下为 no-op）
#[inline]
pub fn send_ipi(_target_cpu: usize, _ipi_type: IpiType) {}

/// 按 hart mask 发送 IPI（单核下为 no-op）
#[inline]
pub fn send_ipi_many(_hart_mask: usize, _ipi_type: IpiType) {}

/// 发送 reschedule IPI（单核下为 no-op）
#[inline]
pub fn send_reschedule_ipi(cpu: usize) {
    send_ipi(cpu, IpiType::Reschedule);
}

/// 向所有 CPU 发送 TLB flush IPI（单核下为 no-op）
#[inline]
pub fn send_tlb_flush_ipi_all() {}
