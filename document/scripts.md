# Scripts 工具说明

本目录包含 Comix 内核项目中使用的辅助脚本工具，用于简化构建流程和文档管理。

## 目录

- [make_init_simple_fs.py](#make_init_simple_fspy) - SimpleFS 镜像打包工具
- [rewrite_links.py](#rewrite_linkspy) - 文档链接转换工具

---

## make_init_simple_fs.py

### 概述

将用户程序和目录打包成 SimpleFS 块设备镜像格式，用于 RamDisk 和 SimpleFS 文件系统。

**位置**：`/workspaces/comix/scripts/make_init_simple_fs.py`

### 主要功能

- 递归收集源目录中的文件和目录
- 将它们打包成与 SimpleFS 兼容的结构化二进制镜像格式
- 支持文件打包和目录遍历
- 自动对齐文件名到 4 字节边界，数据到 512 字节（块）边界
- 提供镜像检查/调试功能

### 镜像格式说明

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

### 使用方法

#### 基本用法

```bash
# 打包用户程序（主要用途）
python3 scripts/make_init_simple_fs.py user/bin os/simple_fs.img
```

#### 详细输出模式

```bash
# 显示详细的打包过程
python3 scripts/make_init_simple_fs.py -v user/bin os/simple_fs.img
```

#### 检查现有镜像

```bash
# 查看镜像内容
python3 scripts/make_init_simple_fs.py --inspect os/simple_fs.img
```

### 命令行参数

| 参数 | 类型 | 说明 |
|------|------|------|
| `src_dir` | 位置参数 | 要打包的源目录路径 |
| `output` | 位置参数 | 输出镜像文件路径 |
| `-v, --verbose` | 可选标志 | 显示详细的打包信息 |
| `-i, --inspect IMAGE` | 可选标志 | 检查并显示现有镜像的内容 |

### 文件过滤规则

脚本在打包时会自动跳过以下文件：

- 隐藏文件（以 `.` 开头）
- `Cargo.lock` 和 `Cargo.toml`
- 编译产物（`.d`, `.o`, `.a`, `.rlib`）
- `.gitignore` 文件
- 符号链接

支持的文件类型：
- 普通文件（FILE_TYPE_FILE = 0，权限 0o644）
- 目录（FILE_TYPE_DIR = 1，权限 0o755）

### 与构建系统集成

此脚本已集成到项目的构建流程中：

#### 自动调用
- 在执行 `cargo build` 或 `cargo run` 时，由 `os/build.rs` 自动调用
- 构建内核时会自动打包 `user/bin` 目录

#### 手动调用
```bash
# 手动打包 SimpleFS 镜像
make pack-simple-fs

# 检查 SimpleFS 镜像内容
make inspect-simple-fs
```

### 依赖要求

- Python 3（使用标准库：struct, sys, argparse, pathlib）
- 对用户程序文件有读取权限
- 输出目录可写

### 错误处理

- 如果源目录不存在，创建空镜像
- 优雅地跳过无效文件
- 提供详细的错误信息

---

## rewrite_links.py

### 概述

将 Markdown 文档中的内部路径链接转换为 GitHub 仓库 URL，使 mdBook 文档能够直接链接到 GitHub 上的源代码。

**位置**：`/workspaces/comix/scripts/rewrite_links.py`

### 主要功能

- 递归处理文档目录中的所有 Markdown 文件
- 将内部代码引用链接转换为可点击的 GitHub 仓库链接
- 支持单行和多行范围引用
- 保持链接文本不变，仅更新 URL

### 链接转换模式

#### 输入格式
```markdown
[描述文本](/os/src/path/file.rs)
[描述文本](/os/src/path/file.rs:12)
[描述文本](/os/src/path/file.rs:12-34)
```

#### 输出格式
```markdown
[描述文本](https://github.com/comix-kernel/comix/blob/main/os/src/path/file.rs)
[描述文本](https://github.com/comix-kernel/comix/blob/main/os/src/path/file.rs#L12)
[描述文本](https://github.com/comix-kernel/comix/blob/main/os/src/path/file.rs#L12-L34)
```

### GitHub 配置

- 仓库：`https://github.com/comix-kernel/comix`
- 分支：`main`

### 使用方法

```bash
python3 scripts/rewrite_links.py document/
```

### 命令行参数

| 参数 | 类型 | 说明 |
|------|------|------|
| `document_dir` | 位置参数（必需） | 文档目录路径，必须存在 |

### 支持的链接格式

1. **文件引用**：`[text](/os/src/path/file.rs)` → 链接到整个文件
2. **单行引用**：`[text](/os/src/path/file.rs:12)` → 带 `#L12` 锚点的链接
3. **行范围引用**：`[text](/os/src/path/file.rs:12-34)` → 带 `#L12-L34` 锚点的链接

### 脚本行为

- 递归处理所有 `.md` 文件
- 仅修改包含匹配模式链接的文件
- 在写入前检查是否有变化（避免不必要的磁盘 I/O）
- 提供处理摘要：显示修改的文件数量和转换的链接数量

### 与 CI/CD 集成

此脚本集成在持续集成/部署流程中：

- 作为 GitHub Actions 工作流的一部分（`docs-deployment.yml`）
- 在文档部署时，在 `mdbook build` 之前自动运行
- 确保发布的文档中所有源代码链接都有效且指向 GitHub

### 输出信息

脚本执行时会输出：
- 修改的文件及其链接数量
- 总修改摘要
- 链接转换状态消息

### 依赖要求

- Python 3（使用：re, sys, pathlib）
- 目标目录必须存在且可读写
- Markdown 文件必须使用 `.md` 扩展名

---

## 脚本协作关系

这两个脚本在项目中各司其职：

- **make_init_simple_fs.py**：负责构建时的文件系统打包，将用户程序集成到内核镜像中
- **rewrite_links.py**：负责文档发布时的链接处理，确保在线文档的可用性

它们共同支持项目的构建流程和文档发布流程，一个面向运行时环境，一个面向开发文档。

---

## 常见问题

### Q: 如何添加新的用户程序到镜像？
A: 将程序放入 `user/bin` 目录，然后重新构建内核（`make run` 或 `cargo build`），脚本会自动打包。

### Q: 如何验证镜像内容？
A: 使用 `make inspect-simple-fs` 或直接运行 `python3 scripts/make_init_simple_fs.py --inspect os/simple_fs.img`

### Q: 文档链接转换后能否在本地查看？
A: 转换后的链接指向 GitHub，建议在文档部署前使用原始链接在本地预览。CI/CD 流程会在部署时自动转换链接。
