//! /sys/devices/ 设备层次结构构建器
//!
//! 实现 Linux sysfs 规范的设备树结构,所有设备的真实目录都在 /sys/devices/ 下

use alloc::format;
use alloc::string::ToString;
use alloc::sync::Arc;

use crate::fs::sysfs::device_registry;
use crate::fs::sysfs::inode::{SysfsAttr, SysfsInode};
use crate::vfs::{FileMode, FsError, Inode};

/// 构建 /sys/devices/ 层次结构
pub fn build_platform_devices(root: &Arc<SysfsInode>) -> Result<(), FsError> {
    // 获取 /sys/devices/
    let devices_inode = root.lookup("devices")?;
    let devices_dir = devices_inode
        .downcast_ref::<SysfsInode>()
        .ok_or(FsError::InvalidArgument)?;

    // 创建 /sys/devices/platform/
    let platform_dir = SysfsInode::new_directory(FileMode::from_bits_truncate(0o040000 | 0o555));
    devices_dir.add_child("platform", platform_dir.clone())?;

    // 构建块设备
    build_platform_block_devices(&platform_dir)?;

    // 构建网络设备
    build_platform_net_devices(&platform_dir)?;

    // 构建 TTY 设备
    build_platform_tty_devices(&platform_dir)?;

    // 构建输入设备
    build_platform_input_devices(&platform_dir)?;

    // 构建 RTC 设备
    build_platform_rtc_devices(&platform_dir)?;

    Ok(())
}

/// 构建平台块设备
fn build_platform_block_devices(platform_dir: &Arc<SysfsInode>) -> Result<(), FsError> {
    for dev_info in device_registry::list_block_devices() {
        // 创建设备目录 /sys/devices/platform/vda/
        let dev_dir = SysfsInode::new_directory(FileMode::from_bits_truncate(0o040000 | 0o555));

        // dev 文件: major:minor
        let dev_attr = SysfsAttr {
            name: "dev".to_string(),
            mode: FileMode::from_bits_truncate(0o444),
            show: {
                let maj = dev_info.major;
                let min = dev_info.minor;
                Arc::new(move || Ok(format!("{}:{}\n", maj, min)))
            },
            store: None,
        };
        dev_dir.add_child("dev", SysfsInode::new_attribute(dev_attr))?;

        // uevent 文件
        let uevent_attr = SysfsAttr {
            name: "uevent".to_string(),
            mode: FileMode::from_bits_truncate(0o644),
            show: {
                let maj = dev_info.major;
                let min = dev_info.minor;
                let name = dev_info.name.clone();
                Arc::new(move || {
                    Ok(format!(
                        "MAJOR={}\nMINOR={}\nDEVNAME={}\nDEVTYPE=disk\n",
                        maj, min, name
                    ))
                })
            },
            store: None,
        };
        dev_dir.add_child("uevent", SysfsInode::new_attribute(uevent_attr))?;

        // size 文件: 扇区数 (512 字节)
        let size_attr = SysfsAttr {
            name: "size".to_string(),
            mode: FileMode::from_bits_truncate(0o444),
            show: {
                let dev = dev_info.device.clone();
                Arc::new(move || {
                    let block_size = dev.block_size();
                    let total_blocks = dev.total_blocks();
                    let total_bytes = block_size * total_blocks;
                    let sectors = total_bytes / 512;
                    Ok(format!("{}\n", sectors))
                })
            },
            store: None,
        };
        dev_dir.add_child("size", SysfsInode::new_attribute(size_attr))?;

        // ro 文件: 是否只读
        let ro_attr = SysfsAttr {
            name: "ro".to_string(),
            mode: FileMode::from_bits_truncate(0o444),
            show: Arc::new(|| Ok("0\n".to_string())),
            store: None,
        };
        dev_dir.add_child("ro", SysfsInode::new_attribute(ro_attr))?;

        // removable 文件: 是否可移动
        let removable_attr = SysfsAttr {
            name: "removable".to_string(),
            mode: FileMode::from_bits_truncate(0o444),
            show: Arc::new(|| Ok("0\n".to_string())),
            store: None,
        };
        dev_dir.add_child("removable", SysfsInode::new_attribute(removable_attr))?;

        // stat 文件: I/O 统计信息
        let stat_attr = SysfsAttr {
            name: "stat".to_string(),
            mode: FileMode::from_bits_truncate(0o444),
            show: Arc::new(|| {
                // 格式: read I/Os, read merges, read sectors, read ticks, write I/Os, ...
                // 对于冷启动,返回全零统计
                Ok("       0        0        0        0        0        0        0        0        0        0        0\n".to_string())
            }),
            store: None,
        };
        dev_dir.add_child("stat", SysfsInode::new_attribute(stat_attr))?;

        // 创建 queue/ 子目录
        build_queue_directory(&dev_dir, &dev_info.device)?;

        // 添加到 platform 目录
        platform_dir.add_child(&dev_info.name, dev_dir)?;
    }

    Ok(())
}

/// 构建块设备的 queue/ 子目录
fn build_queue_directory(
    dev_dir: &Arc<SysfsInode>,
    device: &Arc<dyn crate::device::block::BlockDriver>,
) -> Result<(), FsError> {
    let queue_dir = SysfsInode::new_directory(FileMode::from_bits_truncate(0o040000 | 0o555));

    // logical_block_size
    let logical_block_size_attr = SysfsAttr {
        name: "logical_block_size".to_string(),
        mode: FileMode::from_bits_truncate(0o444),
        show: {
            let dev = device.clone();
            Arc::new(move || Ok(format!("{}\n", dev.block_size())))
        },
        store: None,
    };
    queue_dir.add_child(
        "logical_block_size",
        SysfsInode::new_attribute(logical_block_size_attr),
    )?;

    // physical_block_size
    let physical_block_size_attr = SysfsAttr {
        name: "physical_block_size".to_string(),
        mode: FileMode::from_bits_truncate(0o444),
        show: {
            let dev = device.clone();
            Arc::new(move || Ok(format!("{}\n", dev.block_size())))
        },
        store: None,
    };
    queue_dir.add_child(
        "physical_block_size",
        SysfsInode::new_attribute(physical_block_size_attr),
    )?;

    // hw_sector_size
    let hw_sector_size_attr = SysfsAttr {
        name: "hw_sector_size".to_string(),
        mode: FileMode::from_bits_truncate(0o444),
        show: Arc::new(|| Ok("512\n".to_string())),
        store: None,
    };
    queue_dir.add_child(
        "hw_sector_size",
        SysfsInode::new_attribute(hw_sector_size_attr),
    )?;

    // max_sectors_kb
    let max_sectors_kb_attr = SysfsAttr {
        name: "max_sectors_kb".to_string(),
        mode: FileMode::from_bits_truncate(0o644),
        show: Arc::new(|| Ok("1280\n".to_string())), // 默认值
        store: None,
    };
    queue_dir.add_child(
        "max_sectors_kb",
        SysfsInode::new_attribute(max_sectors_kb_attr),
    )?;

    // rotational (0 = SSD, 1 = HDD)
    let rotational_attr = SysfsAttr {
        name: "rotational".to_string(),
        mode: FileMode::from_bits_truncate(0o644),
        show: Arc::new(|| Ok("1\n".to_string())), // 假设为 HDD
        store: None,
    };
    queue_dir.add_child("rotational", SysfsInode::new_attribute(rotational_attr))?;

    dev_dir.add_child("queue", queue_dir)?;
    Ok(())
}

/// 构建平台网络设备
fn build_platform_net_devices(platform_dir: &Arc<SysfsInode>) -> Result<(), FsError> {
    for dev_info in device_registry::list_net_devices() {
        // 创建设备目录 /sys/devices/platform/eth0/
        let dev_dir = SysfsInode::new_directory(FileMode::from_bits_truncate(0o040000 | 0o555));

        // uevent 文件
        let uevent_attr = SysfsAttr {
            name: "uevent".to_string(),
            mode: FileMode::from_bits_truncate(0o644),
            show: {
                let name = dev_info.name.clone();
                let ifindex = dev_info.ifindex;
                Arc::new(move || Ok(format!("INTERFACE={}\nIFINDEX={}\n", name, ifindex)))
            },
            store: None,
        };
        dev_dir.add_child("uevent", SysfsInode::new_attribute(uevent_attr))?;

        // address 文件: MAC 地址
        let address_attr = SysfsAttr {
            name: "address".to_string(),
            mode: FileMode::from_bits_truncate(0o444),
            show: {
                let dev = dev_info.device.clone();
                Arc::new(move || {
                    let mac = dev.mac_address();
                    Ok(format!(
                        "{:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x}\n",
                        mac[0], mac[1], mac[2], mac[3], mac[4], mac[5]
                    ))
                })
            },
            store: None,
        };
        dev_dir.add_child("address", SysfsInode::new_attribute(address_attr))?;

        // mtu 文件
        let mtu_attr = SysfsAttr {
            name: "mtu".to_string(),
            mode: FileMode::from_bits_truncate(0o644),
            show: {
                let dev = dev_info.device.clone();
                Arc::new(move || Ok(format!("{}\n", dev.mtu())))
            },
            store: None,
        };
        dev_dir.add_child("mtu", SysfsInode::new_attribute(mtu_attr))?;

        // operstate 文件
        let operstate_attr = SysfsAttr {
            name: "operstate".to_string(),
            mode: FileMode::from_bits_truncate(0o444),
            show: Arc::new(|| Ok("up\n".to_string())),
            store: None,
        };
        dev_dir.add_child("operstate", SysfsInode::new_attribute(operstate_attr))?;

        // carrier 文件: 物理链接状态
        let carrier_attr = SysfsAttr {
            name: "carrier".to_string(),
            mode: FileMode::from_bits_truncate(0o444),
            show: Arc::new(|| Ok("1\n".to_string())), // 1 = link up
            store: None,
        };
        dev_dir.add_child("carrier", SysfsInode::new_attribute(carrier_attr))?;

        // ifindex 文件: 接口索引
        let ifindex_attr = SysfsAttr {
            name: "ifindex".to_string(),
            mode: FileMode::from_bits_truncate(0o444),
            show: {
                let ifindex = dev_info.ifindex;
                Arc::new(move || Ok(format!("{}\n", ifindex)))
            },
            store: None,
        };
        dev_dir.add_child("ifindex", SysfsInode::new_attribute(ifindex_attr))?;

        // type 文件: 设备类型 (1 = 以太网)
        let type_attr = SysfsAttr {
            name: "type".to_string(),
            mode: FileMode::from_bits_truncate(0o444),
            show: Arc::new(|| Ok("1\n".to_string())), // ARPHRD_ETHER
            store: None,
        };
        dev_dir.add_child("type", SysfsInode::new_attribute(type_attr))?;

        // 添加到 platform 目录
        platform_dir.add_child(&dev_info.name, dev_dir)?;
    }

    Ok(())
}

/// 构建平台 TTY 设备
fn build_platform_tty_devices(platform_dir: &Arc<SysfsInode>) -> Result<(), FsError> {
    for dev_info in device_registry::list_tty_devices() {
        // 创建设备目录 /sys/devices/platform/console/ 等
        let dev_dir = SysfsInode::new_directory(FileMode::from_bits_truncate(0o040000 | 0o555));

        // dev 文件: major:minor
        let dev_attr = SysfsAttr {
            name: "dev".to_string(),
            mode: FileMode::from_bits_truncate(0o444),
            show: {
                let maj = dev_info.major;
                let min = dev_info.minor;
                Arc::new(move || Ok(format!("{}:{}\n", maj, min)))
            },
            store: None,
        };
        dev_dir.add_child("dev", SysfsInode::new_attribute(dev_attr))?;

        // uevent 文件
        let uevent_attr = SysfsAttr {
            name: "uevent".to_string(),
            mode: FileMode::from_bits_truncate(0o644),
            show: {
                let maj = dev_info.major;
                let min = dev_info.minor;
                let name = dev_info.name.clone();
                Arc::new(move || Ok(format!("MAJOR={}\nMINOR={}\nDEVNAME={}\n", maj, min, name)))
            },
            store: None,
        };
        dev_dir.add_child("uevent", SysfsInode::new_attribute(uevent_attr))?;

        // 添加到 platform 目录
        platform_dir.add_child(&dev_info.name, dev_dir)?;
    }

    Ok(())
}

/// 构建平台输入设备
fn build_platform_input_devices(platform_dir: &Arc<SysfsInode>) -> Result<(), FsError> {
    for dev_info in device_registry::list_input_devices() {
        // 创建设备目录 /sys/devices/platform/input0/ 等
        let dev_dir = SysfsInode::new_directory(FileMode::from_bits_truncate(0o040000 | 0o555));

        // uevent 文件
        let uevent_attr = SysfsAttr {
            name: "uevent".to_string(),
            mode: FileMode::from_bits_truncate(0o644),
            show: {
                let name = dev_info.name.clone();
                Arc::new(move || Ok(format!("NAME={}\n", name)))
            },
            store: None,
        };
        dev_dir.add_child("uevent", SysfsInode::new_attribute(uevent_attr))?;

        // name 文件
        let name_attr = SysfsAttr {
            name: "name".to_string(),
            mode: FileMode::from_bits_truncate(0o444),
            show: {
                let name = dev_info.name.clone();
                Arc::new(move || Ok(format!("{}\n", name)))
            },
            store: None,
        };
        dev_dir.add_child("name", SysfsInode::new_attribute(name_attr))?;

        // 添加到 platform 目录
        platform_dir.add_child(&dev_info.name, dev_dir)?;
    }

    Ok(())
}

/// 构建平台 RTC 设备
fn build_platform_rtc_devices(platform_dir: &Arc<SysfsInode>) -> Result<(), FsError> {
    for dev_info in device_registry::list_rtc_devices() {
        // 创建设备目录 /sys/devices/platform/rtc0/ 等
        let dev_dir = SysfsInode::new_directory(FileMode::from_bits_truncate(0o040000 | 0o555));

        // uevent 文件
        let uevent_attr = SysfsAttr {
            name: "uevent".to_string(),
            mode: FileMode::from_bits_truncate(0o644),
            show: {
                let name = dev_info.name.clone();
                Arc::new(move || Ok(format!("RTC_NAME={}\n", name)))
            },
            store: None,
        };
        dev_dir.add_child("uevent", SysfsInode::new_attribute(uevent_attr))?;

        // name 文件
        let name_attr = SysfsAttr {
            name: "name".to_string(),
            mode: FileMode::from_bits_truncate(0o444),
            show: {
                let name = dev_info.name.clone();
                Arc::new(move || Ok(format!("{}\n", name)))
            },
            store: None,
        };
        dev_dir.add_child("name", SysfsInode::new_attribute(name_attr))?;

        // date 文件: 读取当前 RTC 日期（北京时间）
        let date_attr = SysfsAttr {
            name: "date".to_string(),
            mode: FileMode::from_bits_truncate(0o444),
            show: {
                let rtc = dev_info.device.clone();
                Arc::new(move || {
                    let dt = rtc.read_datetime();
                    Ok(format!("{:04}-{:02}-{:02}\n", dt.year, dt.month, dt.day))
                })
            },
            store: None,
        };
        dev_dir.add_child("date", SysfsInode::new_attribute(date_attr))?;

        // time 文件: 读取当前 RTC 时间（北京时间）
        let time_attr = SysfsAttr {
            name: "time".to_string(),
            mode: FileMode::from_bits_truncate(0o444),
            show: {
                let rtc = dev_info.device.clone();
                Arc::new(move || {
                    let dt = rtc.read_datetime();
                    Ok(format!(
                        "{:02}:{:02}:{:02}\n",
                        dt.hour, dt.minute, dt.second
                    ))
                })
            },
            store: None,
        };
        dev_dir.add_child("time", SysfsInode::new_attribute(time_attr))?;

        // 添加到 platform 目录
        platform_dir.add_child(&dev_info.name, dev_dir)?;
    }

    Ok(())
}
