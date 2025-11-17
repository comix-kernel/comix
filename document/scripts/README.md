# Scripts 工具说明

本目录包含 Comix 内核项目中使用的辅助脚本工具，用于简化构建流程、文档管理和代码质量检查。

## 脚本列表

| 脚本 | 类型 | 说明 | 文档 |
|------|------|------|------|
| `make_init_simple_fs.py` | Python | SimpleFS 镜像打包工具 | [详细文档](./make_init_simple_fs.md) |
| `rewrite_links.py` | Python | 文档链接转换工具 | [详细文档](./rewrite_links.md) |
| `style-check.sh` | Bash | 本地代码质量检查工具 | [详细文档](./style-check.md) |

## 快速参考

### 构建相关

```bash
# 打包 SimpleFS 镜像（通常由构建系统自动调用）
python3 scripts/make_init_simple_fs.py user/bin os/simple_fs.img

# 检查镜像内容
make inspect-simple-fs
```

### 文档相关

```bash
# 转换文档中的代码链接为 GitHub 链接
python3 scripts/rewrite_links.py document/
```

### 代码质量检查

```bash
# 运行本地 style 检查（与 CI 一致）
./scripts/style-check.sh
```

## 脚本协作关系

这三个脚本在项目中各司其职：

- **make_init_simple_fs.py**：负责构建时的文件系统打包，将用户程序集成到内核镜像中
- **rewrite_links.py**：负责文档发布时的链接处理，确保在线文档的可用性
- **style-check.sh**：负责本地代码质量检查，确保代码符合项目规范，减少 CI 失败

它们共同支持项目的构建流程、文档发布流程和开发规范，分别面向运行时环境、开发文档和代码质量。

## 常见问题

### Q: 如何添加新的用户程序到镜像？
A: 将程序放入 `user/bin` 目录，然后重新构建内核（`make run` 或 `cargo build`），脚本会自动打包。

### Q: 如何验证镜像内容？
A: 使用 `make inspect-simple-fs` 或直接运行 `python3 scripts/make_init_simple_fs.py --inspect os/simple_fs.img`

### Q: 文档链接转换后能否在本地查看？
A: 转换后的链接指向 GitHub，建议在文档部署前使用原始链接在本地预览。CI/CD 流程会在部署时自动转换链接。

### Q: 如何在本地运行 CI 的 style 检查？
A: 在项目根目录运行 `./scripts/style-check.sh` 即可。该脚本会执行与 CI 完全相同的检查流程。

### Q: style-check.sh 检查失败了怎么办？
A: 根据失败的检查项采取不同措施：
- **格式化失败**：运行 `make fmt` 自动修复
- **编译错误**：修复代码中的语法/类型错误
- **Clippy 警告/错误**：根据提示修改代码以符合最佳实践

## 相关资源

- [项目根目录 Makefile](/Makefile) - 查看可用的构建命令
- [CI 配置](/.github/workflows/ci.yml) - 查看 CI 检查流程
- [文档部署配置](/.github/workflows/docs-deployment.yml) - 查看文档发布流程
