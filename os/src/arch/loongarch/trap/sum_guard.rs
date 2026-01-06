//! SUM 位保护器（LoongArch 占位实现）
//!
//! LoongArch 平台目前未实现用户内存访问的硬件开关，本守卫提供与 RISC-V
//! 版本一致的接口以便上层代码编译。未来接入实际 CSR 控制时可在此处完善。

/// RAII 样式的 SUM 位守卫
#[derive(Debug, Default, Clone)]
pub struct SumGuard {
    was_enabled: bool,
}

impl SumGuard {
    /// 创建新的守卫。
    ///
    /// 当前实现仅记录占位状态，未对硬件寄存器进行修改。
    #[inline]
    pub fn new() -> Self {
        Self { was_enabled: true }
    }

    /// 返回守卫创建前是否认为 SUM 已开启。
    #[inline]
    pub fn was_enabled(&self) -> bool {
        self.was_enabled
    }
}

impl Drop for SumGuard {
    /// 作用域结束时恢复占位状态。
    #[inline]
    fn drop(&mut self) {}
}
