//! 设备号到驱动的硬编码映射
//!
//! 冷插拔系统的简化实现：所有设备号到驱动的映射都通过硬编码规则完成。

use crate::device::{BLK_DRIVERS, Driver, RTC_DRIVERS, SERIAL_DRIVERS};
use crate::vfs::dev::{major, minor};
use alloc::sync::Arc;

/// 标准字符设备 major 号
pub mod chrdev_major {
    pub const MEM: u32 = 1; // /dev/null, /dev/zero 等
    pub const TTY: u32 = 4; // /dev/tty*, /dev/ttyS*
    pub const CONSOLE: u32 = 5; // /dev/console
    pub const MISC: u32 = 10; // /dev/misc/* (rtc=135)
    pub const INPUT: u32 = 13; // /dev/input/*
}

/// MISC 设备 minor 号
pub mod misc_minor {
    pub const RTC: u32 = 135;
}

/// 标准块设备 major 号
pub mod blkdev_major {
    pub const LOOP: u32 = 7; // /dev/loop*
    pub const SCSI_DISK: u32 = 8; // /dev/sd*
    pub const VIRTIO_BLK: u32 = 254; // /dev/vd*
}

/// 查找字符设备驱动（硬编码规则）
///
/// # 参数
/// - `dev`: 设备号
///
/// # 返回
/// - `Some(driver)`: 找到对应驱动
/// - `None`: 未找到或不需要驱动（如内存设备）
pub fn get_chrdev_driver(dev: u64) -> Option<Arc<dyn Driver>> {
    let maj = major(dev);
    let min = minor(dev);

    match maj {
        chrdev_major::MEM => {
            // 内存设备 (/dev/null, /dev/zero 等)
            // 在 CharDeviceFile 中直接处理，无需驱动
            None
        }
        chrdev_major::TTY => {
            // TTY 设备
            if min >= 64 && min < 128 {
                // 串口设备：ttyS0-ttyS63 (minor 64-127)
                let idx = (min - 64) as usize;
                SERIAL_DRIVERS
                    .read()
                    .get(idx)
                    .map(|d| d.clone() as Arc<dyn Driver>)
            } else {
                // 虚拟终端：tty0-tty63 (minor 0-63)
                // 暂不支持
                None
            }
        }
        chrdev_major::CONSOLE => {
            // 控制台设备 (minor 1)
            // 使用第一个串口作为控制台
            SERIAL_DRIVERS
                .read()
                .first()
                .map(|d| d.clone() as Arc<dyn Driver>)
        }
        chrdev_major::MISC => {
            // misc 设备
            if min == misc_minor::RTC {
                // RTC 设备 (/dev/misc/rtc)
                RTC_DRIVERS
                    .read()
                    .first()
                    .map(|d| d.clone() as Arc<dyn Driver>)
            } else {
                None
            }
        }
        _ => None,
    }
}

/// 查找块设备驱动索引（硬编码规则）
///
/// # 参数
/// - `dev`: 设备号
///
/// # 返回
/// - `Some(index)`: BLK_DRIVERS 中的索引
/// - `None`: 未找到
pub fn get_blkdev_index(dev: u64) -> Option<usize> {
    let maj = major(dev);
    let min = minor(dev);

    match maj {
        blkdev_major::VIRTIO_BLK => {
            // VirtIO 块设备：minor 直接对应 BLK_DRIVERS 索引
            // vda=0, vdb=1, vdc=2, ...
            Some(min as usize)
        }
        blkdev_major::SCSI_DISK => {
            // SCSI 磁盘：每个磁盘占用 16 个 minor（0-15 为分区）
            // sda: minor 0-15, sdb: minor 16-31, ...
            let disk_idx = (min / 16) as usize;
            Some(disk_idx)
        }
        blkdev_major::LOOP => {
            // 回环设备：暂不支持
            None
        }
        _ => None,
    }
}
