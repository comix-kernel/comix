//! 进程间通讯模块
//!
//! 提供进程间通讯的实现
//! 包括:
//! 1. 信号
//! 2. 消息队列
//! 3. 管道
//! 4. 共享内存
#![allow(unused)]
mod message;
mod pipe;
mod shared_memory;
mod signal;

pub use message::*;
pub use pipe::*;
pub use shared_memory::*;
pub use signal::*;
