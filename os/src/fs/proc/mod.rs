//! ProcFS - 进程信息伪文件系统
//!
//! 该模块提供了一个与 **Linux /proc 兼容的虚拟文件系统**，用于导出内核和进程信息。
//!
//! # 组件
//!
//! - [`ProcFS`] - 文件系统结构，管理 /proc 目录树
//! - [`ProcInode`] - Inode 实现，支持静态和动态内容
//! - [`ContentGenerator`] - 动态内容生成器 trait
//! - [`generators`] - 内置生成器（meminfo、cpuinfo、uptime 等）
//!
//! # 设计概览
//!
//! ## Generator 模式
//!
//! ProcFS 使用 **Generator 模式** 动态生成文件内容：
//!
//! ```rust
//! pub trait ContentGenerator: Send + Sync {
//!     fn generate(&self) -> Vec<u8>;
//! }
//! ```
//!
//! ## 文件类型
//!
//! - **静态文件**：内容固定，创建时确定
//! - **动态文件**：每次读取时调用 Generator 生成
//! - **动态符号链接**：目标路径动态计算（如 `/proc/self`）
//! - **进程目录**：为每个进程创建 `/proc/[pid]/` 子目录
//!
//! # 导出的信息
//!
//! ## 系统信息
//!
//! - `/proc/meminfo` - 内存使用情况
//! - `/proc/cpuinfo` - CPU 信息
//! - `/proc/uptime` - 系统运行时间
//! - `/proc/mounts` - 挂载点列表
//!
//! ## 进程信息
//!
//! - `/proc/[pid]/stat` - 进程状态（单行格式）
//! - `/proc/[pid]/status` - 详细状态（键值对格式）
//! - `/proc/[pid]/cmdline` - 命令行参数
//!
//! # 使用示例
//!
//! ```rust
//! use crate::fs::init_procfs;
//!
//! // 初始化并挂载 procfs
//! init_procfs()?;
//!
//! // 读取系统信息
//! let meminfo = vfs_load_file("/proc/meminfo")?;
//! let uptime = vfs_load_file("/proc/uptime")?;
//! ```

pub mod generators;
pub mod inode;
pub mod proc;

pub use inode::{ContentGenerator, ProcInode, ProcInodeContent};
pub use proc::ProcFS;
