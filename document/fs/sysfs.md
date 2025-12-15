# SysFS - 系统设备文件系统

## 概述

SysFS 是一个虚拟文件系统，用于导出内核中的设备信息和状态。它提供了一个层次化的设备树视图。

**主要特点**:
- 设备层次结构：反映设备的物理和逻辑关系
- 属性导出：每个设备可导出多个属性文件
- Builder模式：使用Builder构建设备树
- 只读：当前实现仅支持读取

## 架构设计

```mermaid
graph TB
    A[SysFS] -->|root| B[SysfsInode /sys]
    B --> C[/sys/class]
    B --> D[/sys/devices]
    B --> E[/sys/bus]
    
    C --> F[/sys/class/block]
    F --> G[/sys/class/block/vda]
    G --> H[dev属性文件]
    
    style A fill:#DDA0DD
    style B fill:#87CEEB
    style H fill:#90EE90
```

### 设备注册表

```rust
pub struct DeviceRegistry {
    /// 块设备列表:  名称 -> (major, minor)
    block_devices: BTreeMap<String, (u32, u32)>,
    
    /// 字符设备列表: 名称 -> (major, minor)
    char_devices: BTreeMap<String, (u32, u32)>,
}
```

## 目录结构

| 路径 | 内容 | 说明 |
|------|------|------|
| `/sys/class` | 设备类别 | 按功能分类的设备 |
| `/sys/class/block` | 块设备 | vda, vdb等 |
| `/sys/class/net` | 网络设备 | eth0, lo等 |
| `/sys/devices` | 设备树 | 物理设备层次 |
| `/sys/bus` | 总线 | pci, usb等 |

## 使用示例

```rust
// 查询块设备号
let dev_str = vfs_load_file("/sys/class/block/vda/dev")?;
pr_info!("vda device number: {}", String::from_utf8_lossy(&dev_str));

// 列出所有块设备
let block_dir = vfs_lookup("/sys/class/block")?;
let entries = block_dir.inode.readdir()?;
for entry in entries {
    pr_info!("Block device: {}", entry.name);
}
```

## 添加设备

```rust
// 注册块设备到sysfs
pub fn register_block_device(name: &str, major: u32, minor: u32) {
    let registry = DEVICE_REGISTRY.write();
    registry.add_block_device(name, major, minor);
}
```

## 相关资源

- **源代码**: `os/src/fs/sysfs/`
- **Builders**: `os/src/fs/sysfs/builders/`
- [FS模块概览](README.md)
