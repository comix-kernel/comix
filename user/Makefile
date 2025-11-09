# 设置目标架构
TARGET := riscv64gc-unknown-none-elf
BUILD_MODE := release
BUILD_COMMAND := cargo build --$(BUILD_MODE) --target=$(TARGET)

# 获取当前目录下除lib/外的所有子目录
SUB_DIRS := $(shell find . -maxdepth 1 -type d ! -name '.' ! -name 'lib' ! -name 'bin' ! -path './.*')

# 设置输出目录
BIN_DIR := ./bin

# 默认目标
all: $(BIN_DIR) build_all

# 创建bin目录
$(BIN_DIR):
	@mkdir -p $(BIN_DIR)

# 构建所有子目录
build_all: $(SUB_DIRS)

# 为每个子目录定义构建规则
$(SUB_DIRS):
	@echo "Building $@..."
	@cd $@ && $(BUILD_COMMAND)
	@# 查找并移动可执行文件
	@if [ -d "$@/target/$(TARGET)/$(BUILD_MODE)" ]; then \
		for exe in $@/target/$(TARGET)/$(BUILD_MODE)/*; do \
			if [ -f "$$exe" ] && [ -x "$$exe" ]; then \
				echo "Moving $$exe to $(BIN_DIR)/"; \
				mv "$$exe" "$(BIN_DIR)/"; \
			fi; \
		done; \
	fi

# 清理目标
clean:
	@# 清理bin目录
	@rm -rf $(BIN_DIR)

# 重新构建
rebuild: clean all

# 显示帮助信息
help:
	@echo "可用目标:"
	@echo "  all      - 构建所有项目（默认）"
	@echo "  clean    - 清理所有构建文件"
	@echo "  rebuild  - 重新构建所有项目"
	@echo "  help     - 显示此帮助信息"

.PHONY: all build_all clean rebuild help $(SUB_DIRS)