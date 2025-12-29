#!/bin/bash

# Style Check Script
# 本地运行 CI 中的代码质量检查流程
# 对应 .github/workflows/ci.yml 中的 style 检查步骤

# 颜色定义
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
NC='\033[0m' # No Color

# 临时文件用于捕获输出
TEMP_OUTPUT=$(mktemp)
trap "rm -f $TEMP_OUTPUT" EXIT

# 统计变量
TOTAL_WARNINGS=0
TOTAL_ERRORS=0

# 各阶段统计
CHECK_WARNINGS=0
CHECK_ERRORS=0
CHECK_STATUS="✓"

FORMAT_FILES=0
FORMAT_STATUS="✓"

CLIPPY_WARNINGS=0
CLIPPY_ERRORS=0
CLIPPY_STATUS="✓"

# 检查是否在正确的目录
if [ ! -f "os/Cargo.toml" ]; then
    echo -e "${RED}错误: 请在项目根目录运行此脚本${NC}"
    exit 1
fi

echo -e "${BLUE}======================================${NC}"
echo -e "${BLUE}  Comix 代码质量检查 (Style Check)  ${NC}"
echo -e "${BLUE}======================================${NC}"
echo ""

# 切换到 os 目录
cd os

# 步骤 1: Cargo Check
echo -e "${YELLOW}🔍 步骤 1/3: 运行 Cargo Check (快速验证编译)${NC}"
echo "命令: cargo check --target riscv64gc-unknown-none-elf"
echo ""

# 运行 cargo check 并捕获输出
if cargo check --target riscv64gc-unknown-none-elf 2>&1 | tee $TEMP_OUTPUT; then
    # 统计警告和错误
    CHECK_WARNINGS=$(grep -c "warning:" $TEMP_OUTPUT || true)
    CHECK_ERRORS=$(grep -c "error\[E[0-9]" $TEMP_OUTPUT || true)
    TOTAL_WARNINGS=$((TOTAL_WARNINGS + CHECK_WARNINGS))
    TOTAL_ERRORS=$((TOTAL_ERRORS + CHECK_ERRORS))

    echo ""
    if [ $CHECK_WARNINGS -gt 0 ]; then
        echo -e "${CYAN}  📊 Warnings: $CHECK_WARNINGS${NC}"
    fi
    if [ $CHECK_ERRORS -gt 0 ]; then
        echo -e "${RED}  📊 Errors: $CHECK_ERRORS${NC}"
    fi

    if [ $CHECK_ERRORS -eq 0 ]; then
        echo -e "${GREEN}✓ Cargo Check 通过${NC}"
        CHECK_STATUS="✓"
    else
        echo -e "${RED}✗ Cargo Check 失败${NC}"
        CHECK_STATUS="✗"
        exit 1
    fi
else
    echo -e "${RED}✗ Cargo Check 失败${NC}"
    CHECK_STATUS="✗"
    exit 1
fi
echo ""

# 步骤 2: 代码格式化检查
echo -e "${YELLOW}📏 步骤 2/3: 运行代码格式化检查${NC}"
echo "命令: cargo fmt --all -- --check"
echo ""

# 运行 cargo fmt 并捕获输出
if cargo fmt --all -- --check 2>&1 | tee $TEMP_OUTPUT; then
    # 统计需要格式化的文件数
    FORMAT_FILES=$(grep "^Diff in" $TEMP_OUTPUT | wc -l || true)

    echo ""
    if [ $FORMAT_FILES -gt 0 ]; then
        echo -e "${CYAN}  📊 需要格式化的文件: $FORMAT_FILES${NC}"
        echo -e "${RED}✗ 代码格式化检查失败${NC}"
        FORMAT_STATUS="✗"
        echo ""
        echo -e "${YELLOW}提示: 运行 'make fmt' 或 'cargo fmt --all' 来自动修复格式问题${NC}"
        exit 1
    else
        echo -e "${GREEN}✓ 代码格式化检查通过${NC}"
        FORMAT_STATUS="✓"
    fi
else
    FORMAT_FILES=$(grep "^Diff in" $TEMP_OUTPUT | wc -l || true)
    echo ""
    echo -e "${CYAN}  📊 需要格式化的文件: $FORMAT_FILES${NC}"
    echo -e "${RED}✗ 代码格式化检查失败${NC}"
    FORMAT_STATUS="✗"
    echo ""
    echo -e "${YELLOW}提示: 运行 'make fmt' 或 'cargo fmt --all' 来自动修复格式问题${NC}"
    exit 1
fi
echo ""

# 步骤 3: Clippy Lint 检查
echo -e "${YELLOW}🔬 步骤 3/3: 运行 Clippy Lint 检查${NC}"
echo "命令: cargo clippy --target riscv64gc-unknown-none-elf"
echo ""

# 运行 clippy 并捕获输出
if cargo clippy --target riscv64gc-unknown-none-elf 2>&1 | tee $TEMP_OUTPUT; then
    # 统计警告和错误
    CLIPPY_WARNINGS=$(grep -c "warning:" $TEMP_OUTPUT || true)
    CLIPPY_ERRORS=$(grep -c "error\[E[0-9]" $TEMP_OUTPUT || true)
    TOTAL_WARNINGS=$((TOTAL_WARNINGS + CLIPPY_WARNINGS))
    TOTAL_ERRORS=$((TOTAL_ERRORS + CLIPPY_ERRORS))

    echo ""
    if [ $CLIPPY_WARNINGS -gt 0 ]; then
        echo -e "${CYAN}  📊 Warnings: $CLIPPY_WARNINGS${NC}"
    fi
    if [ $CLIPPY_ERRORS -gt 0 ]; then
        echo -e "${RED}  📊 Errors: $CLIPPY_ERRORS${NC}"
    fi

    if [ $CLIPPY_ERRORS -eq 0 ]; then
        echo -e "${GREEN}✓ Clippy 检查通过${NC}"
        CLIPPY_STATUS="✓"
    else
        echo -e "${RED}✗ Clippy 检查失败${NC}"
        CLIPPY_STATUS="✗"
        exit 1
    fi
else
    echo -e "${RED}✗ Clippy 检查失败${NC}"
    CLIPPY_STATUS="✗"
    exit 1
fi
echo ""

# 全部成功
echo -e "${BLUE}======================================${NC}"
echo -e "${GREEN}✓ 所有代码质量检查通过！${NC}"
echo -e "${BLUE}======================================${NC}"
echo ""

# 显示汇总表格
echo -e "${CYAN}📊 检查结果汇总表:${NC}"
echo ""
printf "${BLUE}%-25s %-10s %-12s %-12s${NC}\n" "检查项" "状态" "Warnings" "Errors"
echo "─────────────────────────────────────────────────────────"

# Cargo Check 行
if [ "$CHECK_STATUS" = "✓" ]; then
    STATUS_COLOR=$GREEN
else
    STATUS_COLOR=$RED
fi
if [ $CHECK_WARNINGS -gt 0 ]; then
    WARN_COLOR=$YELLOW
else
    WARN_COLOR=$GREEN
fi
if [ $CHECK_ERRORS -gt 0 ]; then
    ERR_COLOR=$RED
else
    ERR_COLOR=$GREEN
fi
printf "%-25s ${STATUS_COLOR}%-10s${NC} ${WARN_COLOR}%-12s${NC} ${ERR_COLOR}%-12s${NC}\n" \
    "Cargo Check" "$CHECK_STATUS" "$CHECK_WARNINGS" "$CHECK_ERRORS"

# Code Format 行
if [ "$FORMAT_STATUS" = "✓" ]; then
    STATUS_COLOR=$GREEN
else
    STATUS_COLOR=$RED
fi
if [ $FORMAT_FILES -gt 0 ]; then
    FORMAT_COLOR=$RED
else
    FORMAT_COLOR=$GREEN
fi
printf "%-25s ${STATUS_COLOR}%-10s${NC} ${FORMAT_COLOR}%-12s${NC} %-12s\n" \
    "Code Format" "$FORMAT_STATUS" "$FORMAT_FILES files" "-"

# Clippy 行
if [ "$CLIPPY_STATUS" = "✓" ]; then
    STATUS_COLOR=$GREEN
else
    STATUS_COLOR=$RED
fi
if [ $CLIPPY_WARNINGS -gt 0 ]; then
    WARN_COLOR=$YELLOW
else
    WARN_COLOR=$GREEN
fi
if [ $CLIPPY_ERRORS -gt 0 ]; then
    ERR_COLOR=$RED
else
    ERR_COLOR=$GREEN
fi
printf "%-25s ${STATUS_COLOR}%-10s${NC} ${WARN_COLOR}%-12s${NC} ${ERR_COLOR}%-12s${NC}\n" \
    "Clippy Lint" "$CLIPPY_STATUS" "$CLIPPY_WARNINGS" "$CLIPPY_ERRORS"

echo "─────────────────────────────────────────────────────────"

# 总计行
if [ $TOTAL_WARNINGS -gt 0 ]; then
    WARN_COLOR=$YELLOW
else
    WARN_COLOR=$GREEN
fi
if [ $TOTAL_ERRORS -gt 0 ]; then
    ERR_COLOR=$RED
else
    ERR_COLOR=$GREEN
fi
printf "${BLUE}%-25s %-10s ${WARN_COLOR}%-12s${NC} ${ERR_COLOR}%-12s${NC}\n" \
    "总计" "-" "$TOTAL_WARNINGS" "$TOTAL_ERRORS"

echo ""
