# SimpleFS - 简单测试文件系统

## 概述

SimpleFS 是一个轻量级的只读文件系统，用于测试和调试。文件系统镜像在编译时嵌入到内核中，启动时加载到RamDisk。

**主要特点**:
- 编译时嵌入：镜像作为静态数据包含在内核中
- 快速启动：无需外部文件系统
- 测试友好：提供一致的测试环境
- 只读：不支持修改

## 镜像格式

### 镜像结构

```
+------------------+
| Header (512B)    |
| - Magic: RAMDISK |
| - File count     |
+------------------+
| File Entry 1     |
| - Header (32B)   |
| - Name (aligned) |
| - Data (aligned) |
+------------------+
| File Entry 2     |
| ...              |
+------------------+
```

### 文件条目格式

```rust
struct FileEntry {
    magic: u32,           // 0x46494C45 ("FILE")
    name_len: u32,        // 文件名长度
    data_len: u32,        // 数据长度
    file_type: u32,       // 0=文件, 1=目录
    mode: u32,            // 权限位
    // 之后是name（4字节对齐）
    // 之后是data（512字节对齐）
}
```

## 构建流程

### build.rs 脚本

```rust
// build.rs
fn build_simple_fs() {
    let out_dir = env::var("OUT_DIR").unwrap();
    let image_path = format!("{}/simple_fs.img", out_dir);
    
    // 创建镜像
    create_ramdisk_image(&image_path, "user")?;
    
    // 设置环境变量供include_bytes!使用
    println!("cargo:rustc-env=SIMPLE_FS_IMAGE={}", image_path);
}
```

### 嵌入到内核

```rust
// fs/mod.rs
static SIMPLE_FS_IMAGE: &[u8] = include_bytes!(env!("SIMPLE_FS_IMAGE"));

pub fn init_simple_fs() -> Result<(), FsError> {
    // 从静态数据创建RamDisk
    let ramdisk = RamDisk::from_bytes(
        SIMPLE_FS_IMAGE.to_vec(),
        512,  // 块大小
        0     // 偏移
    );
    
    // 加载SimpleFS
    let simplefs = SimpleFs::from_ramdisk(ramdisk)?;
    
    // 挂载为根文件系统
    MOUNT_TABLE.mount(
        Arc::new(simplefs),
        "/",
        MountFlags::empty(),
        Some(String::from("ramdisk0")),
    )?;
    
    Ok(())
}
```

## 使用场景

### 测试环境

```rust
#[test]
fn test_with_simplefs() {
    init_simple_fs().unwrap();
    
    // 测试文件系统操作
    let content = vfs_load_file("/bin/hello").unwrap();
    assert_eq!(content, b"Hello, World!");
}
```

### 预加载用户程序

```bash
# 将用户程序添加到镜像
cp user/target/riscv64gc-unknown-none-elf/release/init user/
cp user/target/riscv64gc-unknown-none-elf/release/sh user/bin/

# 重新构建内核（会自动重建镜像）
make build
```

## 添加文件到镜像

### 方式1：修改构建脚本

```rust
// build.rs
let files = vec![
    ("bin/init", "user/target/.../init"),
    ("bin/sh", "user/target/.../sh"),
    ("etc/rc", "scripts/rc"),
];

for (dest, src) in files {
    add_file_to_image(&mut image, src, dest)?;
}
```

### 方式2：使用目录

```rust
// build.rs
// 将整个目录添加到镜像
add_directory_to_image(&mut image, "user", "/")?;
```

## 限制

1. **只读**: 运行时无法修改
2. **大小限制**: 镜像过大会增加内核体积
3. **重启丢失**: 运行时的修改不会保存

## 对比：SimpleFS vs Ext4

| 特性 | SimpleFS | Ext4 |
|------|----------|------|
| 持久化 | ❌ | ✅ |
| 修改支持 | ❌ | ✅（读写） |
| 启动速度 | 快 | 慢（需块设备） |
| 镜像大小 | 小 | 大 |
| 用途 | 测试 | 生产 |

## 相关资源

- **源代码**: `os/src/fs/simple_fs.rs`
- **构建脚本**: `os/build.rs`
- [FS模块概览](README.md)
