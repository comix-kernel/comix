//! 设备号工具函数
//!
//! 提供 major/minor 设备号的编码和解码。

/// 从 major 和 minor 构造设备号
///
/// Linux 兼容格式：高 32 位为 major，低 32 位为 minor
#[inline]
pub const fn makedev(major: u32, minor: u32) -> u64 {
    ((major as u64) << 32) | (minor as u64)
}

/// 从设备号提取 major
#[inline]
pub const fn major(dev: u64) -> u32 {
    (dev >> 32) as u32
}

/// 从设备号提取 minor
#[inline]
pub const fn minor(dev: u64) -> u32 {
    (dev & 0xFFFFFFFF) as u32
}
