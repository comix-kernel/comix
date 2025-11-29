//! 地址相关工具函数

/// 这是一个安全且常见的 align_down 实现
/// T 必须是整数类型
pub fn align_down(addr: usize, align: usize) -> usize {
    // 对齐值必须是 2 的幂，否则行为可能不正确
    debug_assert!(align.is_power_of_two());

    // 计算当前地址的偏移量 (misalignment)
    let misalign = addr & (align - 1);

    // 返回向下对齐后的地址
    addr - misalign
}
