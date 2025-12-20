# ===============================================
# 环境变量
# ===============================================

# 架构选择: riscv (默认) 或 loongarch
ARCH ?= riscv

# 根据架构设置变量
ifeq ($(ARCH),loongarch)
    TARGET := loongarch64-unknown-none
    TARGET_DIR := target/loongarch64-unknown-none/debug
    PROJECT_DIR := $(TARGET_DIR)/os
    QEMU_RUNNER := ./qemu-loongarch-run.sh
    GDB_BIN := loongarch64-unknown-linux-gnu-gdb
    GDB_ARCH_SETUP :=
else
    TARGET := riscv64gc-unknown-none-elf
    TARGET_DIR := target/riscv64gc-unknown-none-elf/debug
    PROJECT_DIR := $(TARGET_DIR)/os
    QEMU_RUNNER := ./qemu-run.sh
    GDB_BIN := riscv64-unknown-elf-gdb
    GDB_ARCH_SETUP := -ex "set arch riscv:rv64"
endif

# ===============================================
# 核心目标
# ===============================================
# 注意：cargo build 会自动通过 build.rs 完成以下步骤：
#   1. 编译 ../user 目录中的用户程序
#   2. 打包 user/bin/ 到 simple_fs.img
#   3. 嵌入镜像到内核二进制中
# 因此只需运行 cargo build 即可完成所有构建步骤

# 目标 1: run (默认启动 QEMU)
run:
	@echo "Running $(PROJECT_DIR) [ARCH=$(ARCH)]..."
ifeq ($(TEST), 1)
	@echo "TEST=1 detected. Running tests instead of 'cargo run'."
	@cargo test --target $(TARGET)
else
	@cargo build --target $(TARGET) && $(QEMU_RUNNER) $(PROJECT_DIR) run
endif

# 目标 2: test (显式运行测试)
test:
	@cargo test --target $(TARGET)

.PHONY: run test debug gdb qemu-gdb-target help

# 目标 3: qemu-gdb-target (启动 QEMU 等待 GDB)
# 在后台启动 QEMU 并等待连接，使用 'gdb' 模式
qemu-gdb-target:
	@echo "Starting QEMU in GDB mode (port 1234). Use 'make gdb' to connect."
	@cargo build --target $(TARGET)
	@$(QEMU_RUNNER) $(PROJECT_DIR) gdb &

# 目标 4: gdb (连接到正在运行的 QEMU)
gdb:
	@echo "Starting GDB client and connecting to QEMU on tcp:1234..."
	$(GDB_BIN) \
		-ex "file $(PROJECT_DIR)" \
		-ex "target remote :1234" \
        -ex "b *0x80200000" \
		-ex "dir ../.ignore/busybox" \
		-ex "dir ../.ignore/riscv-musl" \
		-ex "add-symbol-file ../data/bin/busybox"\
        -ex "c"

# 目标 4: debug (一键启动，QEMU在前台，需要另一个终端运行gdb)
debug:
	@cargo build --target $(TARGET)
	@echo "======================================================="
	@echo "在新的终端窗口中运行 'make gdb ARCH=$(ARCH)' 来连接到 QEMU。"
	@echo "QEMU 正在等待 GDB 连接..."
	@echo "======================================================="
	@$(QEMU_RUNNER) $(PROJECT_DIR) gdb
# ===============================================
# 测试目标
# ===============================================
.PHONY: test test-qemu test-gdb clean-test

# 目标 1: 运行测试
test: clean-test
	@echo "Building tests with 'cargo test --no-run'..."
	@TEST=1 cargo test --target $(TARGET) --no-run
	@TEST_ELF=$$(find $(TARGET_DIR)/deps -name "os-*" ! -name "*.d" ! -name "*.bin" -type f -executable -printf '%T@ %p\n' 2>/dev/null | sort -n | tail -1 | cut -d' ' -f2-); \
	if [ -z "$$TEST_ELF" ]; then \
		echo "Error: Could not find any test executable after build."; \
		exit 1; \
	fi; \
	echo "Found latest test executable: $$TEST_ELF"; \
	echo "Running tests in QEMU..."; \
	$(QEMU_RUNNER) $$TEST_ELF run

# 目标 2: 调试测试 (第一步)
test-qemu: clean-test
	@echo "Building tests with 'cargo test --no-run'..."
	@TEST=1 cargo test --target $(TARGET) --no-run
	@# 关键修复：把所有逻辑都放在一个 shell 进程中执行
	@TEST_ELF=$$(find $(TARGET_DIR)/deps -name "os-*" ! -name "*.d" ! -name "*.bin" -type f -executable -printf '%T@ %p\n' 2>/dev/null | sort -n | tail -1 | cut -d' ' -f2-); \
	if [ -z "$$TEST_ELF" ]; then \
		echo "Error: Could not find test executable after build."; \
		exit 1; \
	fi; \
	echo "Found test executable for debugging: $$TEST_ELF"; \
	echo "======================================================="; \
	echo "在新的终端窗口中运行 'make test-gdb ARCH=$(ARCH)' 来连接到 QEMU。"; \
	echo "QEMU 正在等待 GDB 连接..."; \
	echo "======================================================="; \
	$(QEMU_RUNNER) $$TEST_ELF gdb

# 目标 3: 调试测试 (第二步)
test-gdb:
	@TEST_ELF=$$(find $(TARGET_DIR)/deps -name "os-*" ! -name "*.d" ! -name "*.bin" -type f -executable -printf '%T@ %p\n' 2>/dev/null | sort -n | tail -1 | cut -d' ' -f2-); \
	if [ -z "$$TEST_ELF" ]; then \
		echo "Error: Could not find test executable. Did you run 'make test-qemu' first?"; \
		exit 1; \
	fi; \
	echo "Connecting GDB to debug test: $$TEST_ELF"; \
	$(GDB_BIN) \
		-ex "file $$TEST_ELF" \
		-ex "target remote :1234" \
		$(GDB_ARCH_SETUP) \
		-ex "b *0x80200000" \
		-ex "c"

# 目标 4: 快速代码风格检查
quick_check_style:
	cargo clippy --target $(TARGET) -- -A warnings
	cargo fmt --all -- --check

# ===============================================
# 清理目标
# ===============================================
#会删除所有旧的测试可执行文件和 .bin 文件
clean-test:
	@echo "Cleaning old test artifacts (executables and .bin files)..."
	@find $(TARGET_DIR)/deps -name "os-*.bin" -type f -delete 2>/dev/null || true
	@find $(TARGET_DIR)/deps -name "os-*" ! -name "*.d" -type f -executable -delete 2>/dev/null || true
	@echo "Old test artifacts cleaned."

# ===============================================
# 帮助信息
# ===============================================
help:
	@echo "ComixOS Makefile (os/)"
	@echo ""
	@echo "Usage: make [target] ARCH=[riscv|loongarch]"
	@echo ""
	@echo "Architectures:"
	@echo "  riscv      - RISC-V 64-bit (default)"
	@echo "  loongarch  - LoongArch 64-bit"
	@echo ""
	@echo "Targets:"
	@echo "  run        - Build and run the kernel"
	@echo "  debug      - Build and run with GDB support"
	@echo "  gdb        - Connect GDB to running QEMU"
	@echo "  test       - Run tests"
	@echo "  help       - Show this help message"
	@echo ""
	@echo "Examples:"
	@echo "  make run                  # Run RISC-V kernel"
	@echo "  make run ARCH=loongarch   # Run LoongArch kernel"