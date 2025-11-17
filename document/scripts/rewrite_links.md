# rewrite_links.py

文档链接转换工具

## 概述

将 Markdown 文档中的内部路径链接转换为 GitHub 仓库 URL，使 mdBook 文档能够直接链接到 GitHub 上的源代码。

**位置**：`/workspaces/comix/scripts/rewrite_links.py`

## 主要功能

- 递归处理文档目录中的所有 Markdown 文件
- 将内部代码引用链接转换为可点击的 GitHub 仓库链接
- 支持单行和多行范围引用
- 保持链接文本不变，仅更新 URL

## 链接转换模式

### 输入格式
```markdown
[描述文本](/os/src/path/file.rs)
[描述文本](/os/src/path/file.rs:12)
[描述文本](/os/src/path/file.rs:12-34)
```

### 输出格式
```markdown
[描述文本](https://github.com/comix-kernel/comix/blob/main/os/src/path/file.rs)
[描述文本](https://github.com/comix-kernel/comix/blob/main/os/src/path/file.rs#L12)
[描述文本](https://github.com/comix-kernel/comix/blob/main/os/src/path/file.rs#L12-L34)
```

## GitHub 配置

- **仓库**：`https://github.com/comix-kernel/comix`
- **分支**：`main`

如需修改配置，编辑脚本中的以下常量：
```python
GITHUB_REPO = "https://github.com/comix-kernel/comix"
GITHUB_BRANCH = "main"
```

## 使用方法

### 基本用法

```bash
python3 scripts/rewrite_links.py document/
```

### 命令行参数

| 参数 | 类型 | 说明 |
|------|------|------|
| `document_dir` | 位置参数（必需） | 文档目录路径，必须存在 |

## 支持的链接格式

1. **文件引用**：`[text](/os/src/path/file.rs)` → 链接到整个文件
2. **单行引用**：`[text](/os/src/path/file.rs:12)` → 带 `#L12` 锚点的链接
3. **行范围引用**：`[text](/os/src/path/file.rs:12-34)` → 带 `#L12-L34` 锚点的链接

## 脚本行为

- 递归处理所有 `.md` 文件
- 仅修改包含匹配模式链接的文件
- 在写入前检查是否有变化（避免不必要的磁盘 I/O）
- 提供处理摘要：显示修改的文件数量和转换的链接数量

## 输出示例

```
$ python3 scripts/rewrite_links.py document/
Processing Markdown files in document/...

Modified: document/arch/memory.md (3 links)
Modified: document/fs/simple_fs.md (5 links)
Modified: document/process/scheduler.md (2 links)

Summary:
  Files processed: 15
  Files modified: 3
  Total links converted: 10
```

## 与 CI/CD 集成

此脚本集成在持续集成/部署流程中：

- 作为 GitHub Actions 工作流的一部分（`docs-deployment.yml`）
- 在文档部署时，在 `mdbook build` 之前自动运行
- 确保发布的文档中所有源代码链接都有效且指向 GitHub

### CI 集成示例

```yaml
- name: Convert internal links to GitHub URLs
  run: python3 scripts/rewrite_links.py document/

- name: Build mdBook documentation
  run: mdbook build
```

## 正则表达式模式

脚本使用以下正则表达式匹配链接：

```python
LINK_PATTERN = r'\[([^\]]+)\]\((/[^\)]+\.rs(?::\d+(?:-\d+)?)?)\)'
```

**匹配说明**：
- `[...]` - 链接文本
- `(/...)` - 以 `/` 开头的路径
- `.rs` - Rust 源文件
- `:\d+` - 可选的行号
- `-\d+` - 可选的结束行号

## 依赖要求

- Python 3（使用标准库：re, sys, pathlib）
- 目标目录必须存在且可读写
- Markdown 文件必须使用 `.md` 扩展名

## 技术细节

### 转换逻辑

```python
def convert_link(path_with_line):
    # 示例: /os/src/main.rs:12-34
    if ':' in path_with_line:
        path, line_info = path_with_line.split(':', 1)
        if '-' in line_info:
            start, end = line_info.split('-')
            anchor = f"#L{start}-L{end}"
        else:
            anchor = f"#L{line_info}"
    else:
        path = path_with_line
        anchor = ""

    return f"{GITHUB_REPO}/blob/{GITHUB_BRANCH}{path}{anchor}"
```

### 文件处理流程

1. 递归遍历目录，找到所有 `.md` 文件
2. 读取文件内容
3. 使用正则表达式查找匹配的链接
4. 转换链接为 GitHub URL
5. 检查是否有变化
6. 如有变化，写回文件
7. 统计并报告处理结果

## 故障排查

### 问题：脚本没有修改任何文件

**可能原因**：
- 文档中没有符合格式的链接
- 链接已经是 GitHub URL

**解决方法**：
- 检查链接格式是否正确
- 确保链接以 `/` 开头
- 确保文件扩展名是 `.rs`

### 问题：转换后的链接指向错误的仓库

**可能原因**：
- GitHub 配置常量不正确

**解决方法**：
修改脚本中的配置：
```python
GITHUB_REPO = "https://github.com/your-org/your-repo"
GITHUB_BRANCH = "your-branch"
```

### 问题：某些链接没有被转换

**可能原因**：
- 链接格式不符合正则表达式模式
- 不是 `.rs` 文件

**解决方法**：
- 检查链接格式
- 如需支持其他文件类型，修改正则表达式

## 使用建议

### 本地开发

在本地编写文档时，建议使用原始的内部路径链接：
```markdown
[内存管理](/os/src/mm/mod.rs)
```

这样在本地 mdBook 预览时可以正常工作。

### 部署前

在部署文档前（通常由 CI 自动完成），运行转换脚本：
```bash
python3 scripts/rewrite_links.py document/
mdbook build
```

### 版本控制

**不要提交**转换后的文档到版本控制系统。保持文档中使用原始链接，让 CI 在部署时自动转换。

理由：
- 保持文档的可移植性
- 便于本地预览
- 避免不必要的 diff

## 扩展功能

### 支持其他文件类型

修改正则表达式以支持更多文件类型：

```python
# 支持 .rs, .toml, .md 等
LINK_PATTERN = r'\[([^\]]+)\]\((/[^\)]+\.(?:rs|toml|md)(?::\d+(?:-\d+)?)?)\)'
```

### 支持相对路径

如需支持相对路径链接，添加处理逻辑：

```python
if path.startswith('/'):
    # 绝对路径
    github_url = f"{GITHUB_REPO}/blob/{GITHUB_BRANCH}{path}"
else:
    # 相对路径
    github_url = resolve_relative_path(path, current_file)
```

## 相关文档

- [Scripts 工具总览](./README.md)
- [文档部署流程](/.github/workflows/docs-deployment.yml)
- [mdBook 配置](/book.toml)

## 参考资源

- [GitHub URL 格式](https://docs.github.com/en/repositories/working-with-files/using-files/getting-permanent-links-to-files)
- [Python re 模块](https://docs.python.org/3/library/re.html)
- [mdBook 文档](https://rust-lang.github.io/mdBook/)
