//! 设备号工具函数
//!
//! 提供 major/minor 设备号的编码和解码。

/// 从 major 和 minor 构造设备号
///
/// Linux 标准格式: (minor & 0xff) | ((major & 0xfff) << 8) | ((minor & ~0xff) << 12) | ((major & ~0xfff) << 32)
/// 对于 makedev(1, 3) 结果是 0x103
#[inline]
pub const fn makedev(major: u32, minor: u32) -> u64 {
    let major = major as u64;
    let minor = minor as u64;
    (minor & 0xff) | ((major & 0xfff) << 8) | ((minor & !0xff) << 12) | ((major & !0xfff) << 32)
}

/// 从设备号提取 major
#[inline]
pub const fn major(dev: u64) -> u32 {
    (((dev >> 8) & 0xfff) | ((dev >> 32) & !0xfff)) as u32
}

/// 从设备号提取 minor
#[inline]
pub const fn minor(dev: u64) -> u32 {
    ((dev & 0xff) | ((dev >> 12) & !0xff)) as u32
}
