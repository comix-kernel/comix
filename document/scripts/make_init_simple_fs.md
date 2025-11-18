# make_init_simple_fs.py

SimpleFS 镜像打包工具

## 概述

将用户程序和目录打包成 SimpleFS 块设备镜像格式，用于 RamDisk 和 SimpleFS 文件系统。

**位置**：`/workspaces/comix/scripts/make_init_simple_fs.py`

## 主要功能

- 递归收集源目录中的文件和目录
- 将它们打包成与 SimpleFS 兼容的结构化二进制镜像格式
- 支持文件打包和目录遍历
- 自动对齐文件名到 4 字节边界，数据到 512 字节（块）边界
- 提供镜像检查/调试功能

## 镜像格式说明

生成的镜像采用以下二进制格式：

1. **头部** (16 字节)：
   - Magic 字符串：`"RAMDISK\0"` (8 字节)
   - 文件数量：4 字节（小端序）
   - 保留字段：4 字节

2. **文件条目**：
   - 文件 magic：4 字节 (0x46494C45, "FILE" 的 ASCII 码)
   - 文件名长度 + 数据长度 + 文件类型 + 文件权限 + 填充

3. **文件名**：UTF-8 编码，4 字节对齐

4. **文件数据**：原始字节，512 字节对齐

## 使用方法

### 基本用法

```bash
# 打包用户程序（主要用途）
python3 scripts/make_init_simple_fs.py user/bin os/simple_fs.img
```

### 详细输出模式

```bash
# 显示详细的打包过程
python3 scripts/make_init_simple_fs.py -v user/bin os/simple_fs.img
```

### 检查现有镜像

```bash
# 查看镜像内容
python3 scripts/make_init_simple_fs.py --inspect os/simple_fs.img
```

## 命令行参数

| 参数 | 类型 | 说明 |
|------|------|------|
| `src_dir` | 位置参数 | 要打包的源目录路径 |
| `output` | 位置参数 | 输出镜像文件路径 |
| `-v, --verbose` | 可选标志 | 显示详细的打包信息 |
| `-i, --inspect IMAGE` | 可选标志 | 检查并显示现有镜像的内容 |

## 文件过滤规则

脚本在打包时会自动跳过以下文件：

- 隐藏文件（以 `.` 开头）
- `Cargo.lock` 和 `Cargo.toml`
- 编译产物（`.d`, `.o`, `.a`, `.rlib`）
- `.gitignore` 文件
- 符号链接

支持的文件类型：
- 普通文件（FILE_TYPE_FILE = 0，权限 0o644）
- 目录（FILE_TYPE_DIR = 1，权限 0o755）

## 与构建系统集成

此脚本已集成到项目的构建流程中：

### 自动调用
- 在执行 `cargo build` 或 `cargo run` 时，由 `os/build.rs` 自动调用
- 构建内核时会自动打包 `user/bin` 目录

### 手动调用
```bash
# 手动打包 SimpleFS 镜像
make pack-simple-fs

# 检查 SimpleFS 镜像内容
make inspect-simple-fs
```

## 输出示例

### 详细模式输出

```
$ python3 scripts/make_init_simple_fs.py -v user/bin os/simple_fs.img
Collecting files from user/bin...
  Found: hello
  Found: test_app
  Found: calculator
Creating SimpleFS image...
  Writing header: 3 files
  Writing file: hello (8192 bytes)
  Writing file: test_app (4096 bytes)
  Writing file: calculator (12288 bytes)
Image created successfully: os/simple_fs.img (24576 bytes)
```

### 检查模式输出

```
$ python3 scripts/make_init_simple_fs.py --inspect os/simple_fs.img
SimpleFS Image Inspection
=========================
Magic: RAMDISK
Total files: 3

File #1:
  Name: hello
  Type: FILE
  Permission: 0o644
  Size: 8192 bytes

File #2:
  Name: test_app
  Type: FILE
  Permission: 0o644
  Size: 4096 bytes

File #3:
  Name: calculator
  Type: FILE
  Permission: 0o644
  Size: 12288 bytes
```

## 依赖要求

- Python 3（使用标准库：struct, sys, argparse, pathlib）
- 对用户程序文件有读取权限
- 输出目录可写

## 错误处理

- 如果源目录不存在，创建空镜像
- 优雅地跳过无效文件
- 提供详细的错误信息

## 故障排查

### 问题：打包后的镜像无法被内核识别

**可能原因**：
- 镜像格式错误
- 文件对齐问题

**解决方法**：
```bash
# 使用 --inspect 检查镜像内容
python3 scripts/make_init_simple_fs.py --inspect os/simple_fs.img

# 重新生成镜像
rm os/simple_fs.img
make pack-simple-fs
```

### 问题：某些文件没有被打包

**可能原因**：
- 文件在过滤规则中
- 文件是符号链接

**解决方法**：
- 使用 `-v` 参数查看详细打包过程
- 检查文件是否符合过滤规则
- 将符号链接替换为实际文件

## 技术细节

### 对齐规则

- **文件名对齐**：4 字节边界，使用 0 填充
- **数据对齐**：512 字节（块大小）边界，使用 0 填充

### 文件类型常量

```python
FILE_TYPE_FILE = 0  # 普通文件
FILE_TYPE_DIR = 1   # 目录
```

### 权限常量

```python
FILE_PERM_FILE = 0o644  # 普通文件权限 (rw-r--r--)
FILE_PERM_DIR = 0o755   # 目录权限 (rwxr-xr-x)
```

## 相关文档

- [Scripts 工具总览](./README.md)
- [内核构建流程](/os/build.rs)
- [SimpleFS 文件系统实现](/os/src/fs/)

## 扩展阅读

如需修改镜像格式或添加新的文件类型支持，请参考：
- `os/src/fs/simple_fs.rs` - SimpleFS 内核实现
- `os/build.rs` - 构建脚本集成
