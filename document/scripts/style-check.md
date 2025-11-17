# style-check.sh

本地代码质量检查工具

## 概述

本地运行 CI 中的代码质量检查流程，方便开发者在提交代码前进行验证，避免 CI 失败。

**位置**：`/workspaces/comix/scripts/style-check.sh`

## 主要功能

- 运行 `cargo check` 进行快速编译验证
- 运行 `cargo fmt --all -- --check` 进行代码格式化检查
- 运行 `cargo clippy` 进行 Lint 检查
- 统计并显示各阶段的 warnings、errors 和需要格式化的文件数
- 生成美观的汇总表格展示检查结果

## 检查项说明

### 1. Cargo Check (快速验证编译)
- **目的**：验证代码是否能够成功编译
- **命令**：`cargo check --target riscv64gc-unknown-none-elf`
- **统计**：Warnings 和 Errors 数量

### 2. Code Format (代码格式化检查)
- **目的**：检查代码是否符合 rustfmt 标准
- **命令**：`cargo fmt --all -- --check`
- **统计**：需要格式化的文件数量
- **修复方法**：运行 `make fmt` 或 `cargo fmt --all`

### 3. Clippy Lint (代码质量检查)
- **目的**：检查代码中的潜在问题和不规范写法
- **命令**：`cargo clippy --target riscv64gc-unknown-none-elf`
- **统计**：Warnings 和 Errors 数量

## 使用方法

### 基本用法

```bash
# 在项目根目录运行
./scripts/style-check.sh
```

## 输出示例

```
======================================
  Comix 代码质量检查 (Style Check)
======================================

🔍 步骤 1/3: 运行 Cargo Check (快速验证编译)
命令: cargo check --target riscv64gc-unknown-none-elf

    Checking comix-os v0.1.0 (/workspaces/comix/os)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 2.34s

  📊 Warnings: 3
✓ Cargo Check 通过

📏 步骤 2/3: 运行代码格式化检查
命令: cargo fmt --all -- --check

✓ 代码格式化检查通过

🔬 步骤 3/3: 运行 Clippy Lint 检查
命令: cargo clippy --target riscv64gc-unknown-none-elf

    Checking comix-os v0.1.0 (/workspaces/comix/os)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 1.23s

  📊 Warnings: 5
✓ Clippy 检查通过

======================================
✓ 所有代码质量检查通过！
======================================

📊 检查结果汇总表:

检查项                    状态         Warnings     Errors
─────────────────────────────────────────────────────────
Cargo Check              ✓           3            0
Code Format              ✓           0 files      -
Clippy Lint              ✓           5            0
─────────────────────────────────────────────────────────
总计                      -           8            0

```

## 脚本特性

- ✅ **与 CI 保持一致**：检查项与 `.github/workflows/ci.yml` 完全对应
- ✅ **彩色输出**：清晰展示每个步骤的状态（绿色=通过，红色=失败，黄色=警告）
- ✅ **实时统计**：捕获并统计每个阶段的 warnings 和 errors
- ✅ **汇总表格**：最后展示美观的表格，一目了然
- ✅ **快速失败**：遇到错误立即停止，并给出修复提示
- ✅ **自动清理**：使用临时文件捕获输出，脚本结束时自动清理

## 返回值说明

| 返回值 | 说明 |
|--------|------|
| `0` | 所有检查通过 |
| `1` | 至少有一项检查失败 |

## 与 CI/CD 集成

此脚本的检查项与 CI 工作流完全对应：

- **CI 配置文件**：`.github/workflows/ci.yml`
- **检查步骤**：
  1. Cargo Check (`run-tests` job, step "🔍 Cargo Check")
  2. Code Formatting (`run-tests` job, step "📏 Run Code Formatting Check")
  3. Clippy Lint (`run-tests` job, step "🔬 Run Clippy Lint Check")

### CI 对应关系

| 本地脚本步骤 | CI 步骤 | 命令 |
|-------------|--------|------|
| 步骤 1: Cargo Check | 🔍 Cargo Check (快速验证编译) | `cargo check --target riscv64gc-unknown-none-elf` |
| 步骤 2: Code Format | 📏 Run Code Formatting Check | `cargo fmt --all -- --check` |
| 步骤 3: Clippy Lint | 🔬 Run Clippy Lint Check | `cargo clippy --target riscv64gc-unknown-none-elf` |

## 建议工作流

```bash
# 1. 编写代码
vim os/src/main.rs

# 2. 运行本地检查
./scripts/style-check.sh

# 3. 如果格式化检查失败，自动修复
make fmt

# 4. 再次运行检查确保通过
./scripts/style-check.sh

# 5. 提交代码
git add .
git commit -m "feat(xxx): 实现新功能"
git push
```

## 依赖要求

- Bash shell
- Rust toolchain：`nightly-2025-10-28`
- Rust target：`riscv64gc-unknown-none-elf`
- Rust components：`rustfmt`, `clippy`, `rust-src`, `llvm-tools-preview`
- 项目根目录下必须有 `os/Cargo.toml`

### 安装依赖

```bash
# 安装 Rust toolchain
rustup toolchain install nightly-2025-10-28

# 添加 target
rustup target add riscv64gc-unknown-none-elf --toolchain nightly-2025-10-28

# 添加 components
rustup component add rust-src rustfmt clippy llvm-tools-preview --toolchain nightly-2025-10-28
```

## 错误处理

脚本会在以下情况下退出并返回错误码 1：

1. 不在项目根目录运行
2. Cargo Check 发现编译错误
3. 代码格式化检查失败（有文件需要格式化）
4. Clippy 检查发现错误级别的问题

## 故障排查

### 问题：脚本提示 "错误: 请在项目根目录运行此脚本"

**解决方法**：
```bash
# 确保在项目根目录
cd /workspaces/comix
./scripts/style-check.sh
```

### 问题：代码格式化检查失败

**症状**：
```
✗ 代码格式化检查失败

  📊 需要格式化的文件: 3

提示: 运行 'make fmt' 或 'cargo fmt --all' 来自动修复格式问题
```

**解决方法**：
```bash
# 自动修复格式问题
make fmt
# 或
cd os && cargo fmt --all

# 重新运行检查
./scripts/style-check.sh
```

### 问题：Clippy 检查失败

**症状**：
```
  📊 Warnings: 0
  📊 Errors: 2
✗ Clippy 检查失败
```

**解决方法**：
1. 仔细阅读 Clippy 的错误信息
2. 修复代码中的问题
3. 重新运行检查

**常见 Clippy 问题**：
- 未使用的变量：添加 `_` 前缀或使用 `#[allow(unused)]`
- 不必要的克隆：使用引用替代
- 复杂的条件表达式：简化逻辑

### 问题：Cargo Check 编译错误

**症状**：
```
  📊 Errors: 5
✗ Cargo Check 失败
```

**解决方法**：
1. 查看详细的编译错误信息
2. 修复类型错误、语法错误等
3. 重新运行检查

## 技术实现

### 统计机制

脚本使用临时文件捕获命令输出，然后使用 `grep` 统计：

```bash
# 捕获输出
cargo check 2>&1 | tee $TEMP_OUTPUT

# 统计 warnings 和 errors
WARNINGS=$(grep -c "warning:" $TEMP_OUTPUT || true)
ERRORS=$(grep -c "error:" $TEMP_OUTPUT || true)
```

### 表格生成

使用 `printf` 格式化输出表格，根据数值动态设置颜色：

```bash
if [ $WARNINGS -gt 0 ]; then
    WARN_COLOR=$YELLOW
else
    WARN_COLOR=$GREEN
fi

printf "%-25s ${STATUS_COLOR}%-10s${NC} ${WARN_COLOR}%-12s${NC}\n" \
    "Cargo Check" "$CHECK_STATUS" "$CHECK_WARNINGS"
```

### 自动清理

使用 `trap` 确保临时文件被清理：

```bash
TEMP_OUTPUT=$(mktemp)
trap "rm -f $TEMP_OUTPUT" EXIT
```

## 扩展功能

### 添加新的检查项

在脚本中添加新的检查步骤：

```bash
# 步骤 4: 运行测试
echo -e "${YELLOW}🧪 步骤 4/4: 运行测试${NC}"
echo "命令: cargo test"
echo ""

if cargo test 2>&1 | tee $TEMP_OUTPUT; then
    echo -e "${GREEN}✓ 测试通过${NC}"
else
    echo -e "${RED}✗ 测试失败${NC}"
    exit 1
fi
```

### 支持并行检查

使用后台进程并行运行检查：

```bash
# 并行运行（需要修改脚本逻辑）
cargo check &
cargo fmt --all -- --check &
cargo clippy &

# 等待所有任务完成
wait
```

### 添加配置文件

创建 `.style-check.conf` 支持自定义配置：

```bash
# 配置文件示例
TARGET=riscv64gc-unknown-none-elf
CLIPPY_ARGS="-- -D warnings"
FMT_ARGS="--all"
```

## 性能优化

### 利用缓存

脚本每次运行都会利用 Cargo 的增量编译缓存，通常第二次运行会快很多。

### 选择性检查

如果只想运行特定检查，可以修改脚本或创建单独的脚本：

```bash
# 仅检查格式化
cd os && cargo fmt --all -- --check

# 仅运行 Clippy
cd os && cargo clippy --target riscv64gc-unknown-none-elf
```

## 相关文档

- [Scripts 工具总览](./README.md)
- [CI 配置](/.github/workflows/ci.yml)
- [Rust 代码规范](https://doc.rust-lang.org/nightly/style-guide/)
- [Clippy Lints](https://rust-lang.github.io/rust-clippy/master/)

## 最佳实践

1. **提交前检查**：每次提交代码前运行此脚本
2. **Pre-commit Hook**：考虑将脚本集成到 Git pre-commit hook
3. **CI/CD 对齐**：确保本地检查与 CI 保持一致
4. **及时修复**：发现问题立即修复，不要累积

## Git Hook 集成

创建 `.git/hooks/pre-commit` 文件：

```bash
#!/bin/bash
# Pre-commit hook: 运行 style 检查

echo "Running style checks..."
./scripts/style-check.sh

if [ $? -ne 0 ]; then
    echo "Style checks failed. Commit aborted."
    exit 1
fi

echo "All checks passed. Proceeding with commit."
```

设置执行权限：
```bash
chmod +x .git/hooks/pre-commit
```
