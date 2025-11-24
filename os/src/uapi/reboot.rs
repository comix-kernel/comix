//! Reboot 常量和定义
//!
//! 对应于 Linux 用户空间 API 定义。

// 魔数，用于验证调用 `reboot` 系统调用的意图。

/// 第一个魔数。
pub const REBOOT_MAGIC1: u32 = 0xfee1dead;

// 第二个魔数有多个版本，用于历史兼容性或特定架构。

/// 第二个魔数（常见值）。
pub const REBOOT_MAGIC2: u32 = 672274793;

/// 第二个魔数（版本 A）。
pub const REBOOT_MAGIC2A: u32 = 85072278;

/// 第二个魔数（版本 B）。
pub const REBOOT_MAGIC2B: u32 = 369367448;

/// 第二个魔数（版本 C）。
pub const REBOOT_MAGIC2C: u32 = 537993216;

// `reboot` 系统调用接受的操作码。

/// RESTART: 使用默认命令和模式重启系统。
pub const REBOOT_CMD_RESTART: u32 = 0x01234567;

/// HALT: 停止操作系统，并将系统控制权交给 ROM 监视器（如果存在）。
pub const REBOOT_CMD_HALT: u32 = 0xCDEF0123;

/// POWER_OFF: 停止操作系统，并在可能的情况下切断系统所有电源（关机）。
pub const REBOOT_CMD_POWER_OFF: u32 = 0x4321FEDC;

/// SW_SUSPEND: 使用软件挂起（S3/休眠）来挂起系统。
pub const REBOOT_CMD_SW_SUSPEND: u32 = 0xD000FCE2;

// Ctrl-Alt-Del 行为

/// CAD_ON: Ctrl-Alt-Del 序列触发 RESTART 命令。
pub const REBOOT_CMD_CAD_ON: u32 = 0x89ABCDEF;

/// CAD_OFF: Ctrl-Alt-Del 序列向 init 任务发送 SIGINT 信号。
pub const REBOOT_CMD_CAD_OFF: u32 = 0x00000000;

// 特殊重启

/// RESTART2: 使用给定的命令字符串重启系统。
pub const REBOOT_CMD_RESTART2: u32 = 0xA1B2C3D4;

/// KEXEC: 使用先前加载的 Linux 内核重启系统（即热启动新内核）。
pub const REBOOT_CMD_KEXEC: u32 = 0x45584543;
