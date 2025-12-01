//! Sysfs 虚拟文件系统
//!
//! 提供与 Linux 兼容的 sysfs 接口,用于暴露设备和内核信息。
//!
//! # 特性
//!
//! - 冷插拔: 设备在启动时注册,运行时不变
//! - 只读: 大部分文件只读 (部分配置文件可写)
//! - 动态生成: 文件内容通过闭包动态生成
//! - Linux ABI 兼容: 符合 Linux sysfs 规范
//!
//! # 目录结构
//!
//! ```text
//! /sys/
//! ├── devices/          # 设备层次结构 (真实设备目录)
//! │   └── platform/
//! │       ├── vda/      # 块设备
//! │       │   ├── dev
//! │       │   ├── uevent
//! │       │   ├── size
//! │       │   ├── ro
//! │       │   ├── removable
//! │       │   ├── stat
//! │       │   └── queue/
//! │       │       ├── logical_block_size
//! │       │       ├── physical_block_size
//! │       │       ├── hw_sector_size
//! │       │       ├── max_sectors_kb
//! │       │       └── rotational
//! │       └── eth0/     # 网络设备
//! │           ├── uevent
//! │           ├── address
//! │           ├── mtu
//! │           ├── operstate
//! │           ├── carrier
//! │           ├── ifindex
//! │           └── type
//! ├── class/            # 设备分类 (符号链接)
//! │   ├── block/
//! │   │   └── vda -> ../../devices/platform/vda
//! │   └── net/
//! │       └── eth0 -> ../../devices/platform/eth0
//! ├── block -> class/block/  # 向后兼容
//! └── kernel/           # 内核信息
//!     ├── version
//!     └── osrelease
//! ```
//!
//! # Linux ABI 兼容性
//!
//! 实现了以下 Linux ABI 稳定属性:
//!
//! ## 块设备
//! - `dev`: major:minor 设备号
//! - `uevent`: udev 事件文件
//! - `size`: 设备大小(512字节扇区数)
//! - `ro`: 只读标志
//! - `removable`: 可移动标志
//! - `stat`: I/O 统计信息
//! - `queue/logical_block_size`: 逻辑块大小
//! - `queue/physical_block_size`: 物理块大小
//! - `queue/hw_sector_size`: 硬件扇区大小
//! - `queue/max_sectors_kb`: 最大传输大小
//! - `queue/rotational`: 旋转设备标志
//!
//! ## 网络设备
//! - `uevent`: udev 事件文件
//! - `address`: MAC 地址
//! - `mtu`: 最大传输单元
//! - `operstate`: 操作状态
//! - `carrier`: 载波状态
//! - `ifindex`: 接口索引
//! - `type`: 设备类型

mod builders;
mod device_registry;
mod inode;
mod sysfs;

pub use device_registry::{find_block_device, find_net_device};
pub use sysfs::SysFS;
