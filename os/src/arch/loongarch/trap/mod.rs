//! LoongArch64 陷阱/异常处理模块（存根）

pub mod trap_frame;

pub use trap_frame::TrapFrame;

/// 用户内存访问守卫
pub struct SumGuard;

impl SumGuard {
    /// 创建新的守卫，允许访问用户内存
    pub fn new() -> Self {
        // TODO: 实现 LoongArch 用户内存访问控制
        Self
    }
}

impl Default for SumGuard {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for SumGuard {
    fn drop(&mut self) {
        // TODO: 恢复用户内存访问控制
    }
}

/// 初始化启动阶段陷阱处理
pub fn init_boot_trap() {
    // TODO: 设置 LoongArch 异常入口
}

/// 初始化陷阱处理
pub fn init() {
    // TODO: 设置 LoongArch 异常入口
}

/// 恢复陷阱帧
pub fn restore(tf: &TrapFrame) -> ! {
    let _ = tf;
    // TODO: 实现陷阱帧恢复
    loop {
        unsafe { core::arch::asm!("idle 0") };
    }
}

/// 获取信号返回 trampoline 地址
pub fn sigreturn_trampoline_address() -> usize {
    // TODO: 实现信号返回 trampoline
    0
}
