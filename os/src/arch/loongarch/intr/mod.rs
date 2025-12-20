//! LoongArch64 中断处理模块（存根）

/// 启用中断
/// # Safety
/// 直接操作 CSR 寄存器
pub unsafe fn enable_interrupts() {
    // TODO: 实现 LoongArch 中断启用
}

/// 禁用中断
/// # Safety
/// 直接操作 CSR 寄存器
pub unsafe fn disable_interrupts() {
    // TODO: 实现 LoongArch 中断禁用
}

/// 检查中断是否启用
pub fn is_interrupts_enabled() -> bool {
    // TODO: 实现
    false
}

/// 检查中断是否启用（别名）
pub fn are_interrupts_enabled() -> bool {
    is_interrupts_enabled()
}

/// 读取并禁用中断（返回之前的状态）
pub fn read_and_disable_interrupts() -> usize {
    // TODO: 实现
    0
}

/// 读取并启用中断
pub fn read_and_enable_interrupts() -> usize {
    // TODO: 实现
    0
}

/// 恢复中断状态
pub fn restore_interrupts(_flags: usize) {
    // TODO: 实现
}

/// 启用指定 IRQ
pub fn enable_irq(_irq: usize) {
    // TODO: 实现
}

/// 禁用指定 IRQ
pub fn disable_irq(_irq: usize) {
    // TODO: 实现
}

/// 软中断模块
pub mod softirq {
    /// 初始化软中断
    pub fn init() {}
}
