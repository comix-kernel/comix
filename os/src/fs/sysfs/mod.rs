//! Sysfs 虚拟文件系统
//!
//! 提供与 Linux 兼容的 sysfs 接口,用于暴露设备和内核信息。
//!
//! # 特性
//!
//! - 冷插拔: 设备在启动时注册,运行时不变
//! - 只读: 大部分文件只读 (部分配置文件可写)
//! - 动态生成: 文件内容通过闭包动态生成
//!
//! # 目录结构
//!
//! ```
//! /sys/
//! ├── class/
//! │   ├── block/      # 块设备
//! │   │   └── vda/
//! │   │       ├── dev
//! │   │       ├── size
//! │   │       ├── ro
//! │   │       └── removable
//! │   └── net/        # 网络设备
//! │       └── eth0/
//! │           ├── address
//! │           ├── mtu
//! │           └── operstate
//! ├── kernel/         # 内核信息
//! │   ├── version
//! │   └── osrelease
//! └── devices/        # 设备层次结构
//! ```

mod builders;
mod device_registry;
mod inode;
mod sysfs;

pub use sysfs::SysFS;
