//! LoongArch64 系统调用号定义
//!
//! 基于 Linux LoongArch ABI

#![allow(dead_code)]

/// 系统调用号定义
pub mod nr {
    pub const READ: usize = 63;
    pub const WRITE: usize = 64;
    pub const EXIT: usize = 93;
    pub const EXIT_GROUP: usize = 94;
    pub const BRK: usize = 214;
    pub const MMAP: usize = 222;
    pub const MUNMAP: usize = 215;
    // TODO: 添加更多系统调用号
}

/// 将系统调用号转换为名称
pub fn syscall_name(id: usize) -> &'static str {
    match id {
        nr::READ => "read",
        nr::WRITE => "write",
        nr::EXIT => "exit",
        nr::EXIT_GROUP => "exit_group",
        nr::BRK => "brk",
        nr::MMAP => "mmap",
        nr::MUNMAP => "munmap",
        _ => "unknown",
    }
}
