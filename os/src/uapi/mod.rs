//! 与用户空间共用定义和声明
//!
//! 包含常量、类型和函数声明，确保内核和用户空间的一致性

#![allow(dead_code)]
pub mod cred;
pub mod errno;
pub mod fcntl;
pub mod fs;
pub mod futex;
pub mod ioctl;
pub mod iovec;
pub mod log;
pub mod mm;
pub mod reboot;
pub mod resource;
pub mod sched;
pub mod select;
pub mod signal;
pub mod socket;
pub mod sysinfo;
pub mod time;
pub mod types;
pub mod uts_namespace;
pub mod wait;
