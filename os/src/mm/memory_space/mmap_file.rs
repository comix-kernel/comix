//! mmap 文件映射信息

use crate::uapi::mm::{MapFlags, ProtFlags};
use crate::vfs::File;
use alloc::sync::Arc;

/// 文件映射信息
pub struct MmapFile {
    /// 文件对象引用（用于权限检查和获取 Inode）
    pub file: Arc<dyn File>,
    /// 文件偏移量（字节）
    pub offset: usize,
    /// 映射长度（字节）
    pub len: usize,
    /// 保护标志
    pub prot: ProtFlags,
    /// 映射标志
    pub flags: MapFlags,
}

// 手动实现 Debug，因为 dyn File 没有实现 Debug
impl core::fmt::Debug for MmapFile {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("MmapFile")
            .field("file", &"<dyn File>")
            .field("offset", &self.offset)
            .field("len", &self.len)
            .field("prot", &self.prot)
            .field("flags", &self.flags)
            .finish()
    }
}
