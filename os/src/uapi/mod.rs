//! 与用户空间共用定义和声明
//!
//! 包含常量、类型和函数声明，确保内核和用户空间的一致性

#![allow(dead_code)]
pub mod errno;
pub mod fcntl;
pub mod fs;
pub mod mm;
pub mod reboot;
pub mod resource;
