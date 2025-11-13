//! 一个简单的内存文件系统（MemFS）实现。
//!
//! 在这个文件系统中，所有文件都嵌入在内核的只读数据段中，
//! 以静态字节切片的形式存在。
//! 在实现真正的文件系统之前，这个模块为用户程序提供了基本的存储功能。
use alloc::string::String;
use alloc::vec::Vec;

/// 描述简单内存文件系统的单个文件条目。
pub struct FileEntry {
    /// 文件名，通常是 UTF-8 字符串
    pub name: &'static str,
    /// 文件的内容，作为静态字节切片嵌入
    pub data: &'static [u8],
    /// 文件大小（冗余字段，但方便快速访问）
    pub size: usize,
}

/// 简单内存文件系统的主要结构，包含所有静态文件条目。
pub struct SimpleMemoryFileSystem {
    files: &'static [FileEntry],
}

// 对齐包装类型：将嵌入的字节数组强制为 8 字节对齐
#[repr(align(8))]
struct Align8<const N: usize>([u8; N]);

// 用 include_bytes! 宏将编译好的用户程序嵌入到这里
const INIT: Align8<{ include_bytes!("../../../user/bin/init").len() }> =
    Align8(*include_bytes!("../../../user/bin/init"));
static HELLO: Align8<{ include_bytes!("../../../user/bin/hello").len() }> =
    Align8(*include_bytes!("../../../user/bin/hello"));

/// 静态文件列表：这是 MemFS 的核心存储
static STATIC_FILES: [FileEntry; 2] = [
    FileEntry {
        name: "init",
        data: &INIT.0,
        size: INIT.0.len(),
    },
    FileEntry {
        name: "hello",
        data: &HELLO.0,
        size: HELLO.0.len(),
    },
];

// 这是为了通过cargo check，实际使用时请仿照上示代码将文件添加进来
// static STATIC_FILES: [FileEntry; 0] = [];

impl SimpleMemoryFileSystem {
    /// # 函数：init
    /// 初始化内存文件系统。
    pub const fn init() -> Self {
        Self {
            files: &STATIC_FILES,
        }
    }

    /// # 函数：lookup
    /// 根据文件名查找文件的内容切片。
    ///
    /// @param name - 要查找的文件名。
    /// @returns - 如果找到，返回文件的内容切片；否则返回 None。
    pub fn lookup(&self, name: &str) -> Option<&'static [u8]> {
        self.files
            .iter()
            .find(|entry| entry.name == name)
            .map(|entry| entry.data)
    }

    /// # 函数：list_all
    /// 获取所有文件的名称列表。
    pub fn list_all(&self) -> Vec<String> {
        self.files
            .iter()
            .map(|entry| String::from(entry.name))
            .collect()
    }

    /// # 辅助函数：load_elf
    /// 这是供进程创建使用的主要接口。
    ///
    /// @param name - ELF 文件名。
    /// @returns - ELF 二进制数据的字节切片。
    pub fn load_elf(&self, name: &str) -> Option<&'static [u8]> {
        self.lookup(name)
    }
}
