//! 内存管理相关的用户空间 API 定义

use bitflags::bitflags;

bitflags! {
    /// 内存保护标志（mmap/mprotect）
    ///
    /// 参考：include/uapi/asm-generic/mman-common.h
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct ProtFlags: i32 {
        /// 页面不可访问 (PROT_NONE)
        const NONE = 0x0;

        /// 页面可读 (PROT_READ)
        const READ = 0x1;

        /// 页面可写 (PROT_WRITE)
        const WRITE = 0x2;

        /// 页面可执行 (PROT_EXEC)
        const EXEC = 0x4;
    }
}

bitflags! {
    /// 内存映射标志（mmap）
    ///
    /// 参考：include/uapi/asm-generic/mman-common.h
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct MapFlags: i32 {
        /// 共享映射（修改对其他进程可见）(MAP_SHARED)
        const SHARED = 0x01;

        /// 私有映射（写时复制）(MAP_PRIVATE)
        const PRIVATE = 0x02;

        /// 映射类型掩码 (MAP_TYPE)
        const TYPE_MASK = 0x0f;

        /// 精确映射到指定地址 (MAP_FIXED)
        const FIXED = 0x10;

        /// 匿名映射（无文件支持）(MAP_ANONYMOUS)
        const ANONYMOUS = 0x20;

        /// 预填充页面 (MAP_POPULATE)
        const POPULATE = 0x008000;

        /// 不阻塞（与 MAP_POPULATE 一起使用）(MAP_NONBLOCK)
        const NONBLOCK = 0x010000;

        /// 为栈分配 (MAP_STACK)
        const STACK = 0x020000;

        /// 使用大页 (MAP_HUGETLB)
        const HUGETLB = 0x040000;

        /// 同步映射（DAX）(MAP_SYNC)
        const SYNC = 0x080000;

        /// 不替换现有映射 (MAP_FIXED_NOREPLACE)
        const FIXED_NOREPLACE = 0x100000;
    }
}

impl MapFlags {
    /// 检查标志组合是否合法
    ///
    /// MAP_SHARED 和 MAP_PRIVATE 必须设置且只能设置一个
    pub fn is_valid(self) -> bool {
        let shared = self.contains(Self::SHARED);
        let private = self.contains(Self::PRIVATE);

        // 必须设置一个，且不能同时设置
        shared != private
    }

    /// 获取映射类型（SHARED 或 PRIVATE）
    pub fn map_type(self) -> Option<MapType> {
        if self.contains(Self::SHARED) {
            Some(MapType::Shared)
        } else if self.contains(Self::PRIVATE) {
            Some(MapType::Private)
        } else {
            None
        }
    }
}

/// 映射类型枚举
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MapType {
    /// 共享映射
    Shared,
    /// 私有映射（写时复制）
    Private,
}

/// MAP_FAILED 常量
///
/// mmap 失败时的返回值
pub const MAP_FAILED: isize = -1;
