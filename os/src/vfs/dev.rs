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

/// 将内核内部设备号编码为 Linux 用户态 ABI 使用的 dev_t。
#[inline]
pub const fn encode_linux_dev(dev: u64) -> u64 {
    let major = major(dev) as u64;
    let minor = minor(dev) as u64;
    ((major & 0x00000fff) << 8)
        | ((major & 0xfffff000) << 32)
        | (minor & 0x000000ff)
        | ((minor & 0xffffff00) << 12)
}

/// 将 Linux 用户态 ABI 的 dev_t 解码为内核内部设备号。
#[inline]
pub const fn decode_linux_dev(dev: u64) -> u64 {
    let major = ((dev & 0x00000000000fff00) >> 8) | ((dev & 0xfffff00000000000) >> 32);
    let minor = (dev & 0x00000000000000ff) | ((dev & 0x00000ffffff00000) >> 12);
    makedev(major as u32, minor as u32)
}

/// ext2/3/4 旧格式设备号编码，保存在 inode 的 i_block[0]。
#[inline]
pub const fn encode_ext4_old_dev(dev: u64) -> u32 {
    let major = major(dev);
    let minor = minor(dev);
    ((major & 0xff) << 8) | (minor & 0xff)
}

/// ext2/3/4 新格式设备号编码，保存在 inode 的 i_block[1]。
#[inline]
pub const fn encode_ext4_new_dev(dev: u64) -> u32 {
    let major = major(dev);
    let minor = minor(dev);
    (minor & 0xff) | ((major & 0xfff) << 8) | ((minor & !0xff) << 12)
}

/// 解码 ext2/3/4 旧格式设备号。
#[inline]
pub const fn decode_ext4_old_dev(encoded: u32) -> u64 {
    makedev((encoded >> 8) & 0xff, encoded & 0xff)
}

/// 解码 ext2/3/4 新格式设备号。
#[inline]
pub const fn decode_ext4_new_dev(encoded: u32) -> u64 {
    let major = (encoded >> 8) & 0xfff;
    let minor = (encoded & 0xff) | ((encoded >> 12) & !0xff);
    makedev(major, minor)
}
