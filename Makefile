DOCKER_TAG ?= comix:latest

# 架构选择: riscv (默认) 或 loongarch
ARCH ?= riscv

# 根据架构设置 target 和运行命令
ifeq ($(ARCH),loongarch)
    TARGET := loongarch64-unknown-none
    TARGET_DIR := target/loongarch64-unknown-none/debug
    PROJECT_DIR := $(TARGET_DIR)/os
    RUN_SCRIPT := cargo run --target $(TARGET)
    GDB_SCRIPT := cargo run --target $(TARGET) -- gdb
else
    TARGET := riscv64gc-unknown-none-elf
    RUN_SCRIPT := cargo run
    GDB_SCRIPT := cargo run -- --gdb
endif

# ============================================================
# OSCOMP 评测提交目标（根 Makefile）
# ============================================================
# 评测机在仓库根执行 `make all`，要求产出 ELF 内核：kernel-rv、kernel-la。
# 评测机 clone 时会过滤隐藏目录（含 os/.cargo），故构建前重建 os/.cargo/config.toml。
RV_TARGET := riscv64gc-unknown-none-elf
LA_TARGET := loongarch64-unknown-none
OS_DIR := os
OS_BIN := os

# 评测构建 profile：默认 release（提交用）。本地调试可 `make all PROFILE=debug`。
PROFILE ?= release
ifeq ($(PROFILE),debug)
    CARGO_PROFILE_FLAG :=
    PROFILE_DIR := debug
else
    CARGO_PROFILE_FLAG := --release
    PROFILE_DIR := release
endif

# 评测内核默认启用 oscomp 特性：使用单盘 rootfs（内含 /tests），
# 由 rootfs 的 rcS 自动跑 musl 测试并主动关机（赛题要求“自动运行 + 自动关闭”）。
# 本地交互式开发请用 `make run`（os/Makefile，不带 oscomp）。
OSCOMP_FEATURE ?= --features oscomp

# 本地复现评测用 QEMU 参数（可在命令行覆盖，如 `make run-oscomp-rv OSCOMP_RV_MEM=2G`）。
# rootfs 用 make all 产出的 disk{,-la}.img，里面已经包含 /tests/{musl,glibc}。
OSCOMP_RV_MEM ?= 4G
OSCOMP_RV_SMP ?= 1
OSCOMP_LA_MEM ?= 4G
OSCOMP_LA_SMP ?= 1

.PHONY: docker build_docker fmt run build clean clean-all gdb
.PHONY: all kernel-rv kernel-la os-cargo-config
.PHONY: run-oscomp-rv run-oscomp-la

docker:
	docker run --rm -it -v ${PWD}:/mnt -w /mnt --name comix ${DOCKER_TAG} bash

build_docker:
	docker build -t ${DOCKER_TAG} --target build .

fmt:
	cd os && cargo fmt

# 构建内核（build.rs 会自动编译 user 并打包镜像）
build:
	cd os && cargo build --target $(TARGET)

# 运行内核（build.rs 会自动编译 user 并打包镜像）
run:
	cd os && $(RUN_SCRIPT)

# GDB 调试模式
gdb:
	cd os && $(GDB_SCRIPT)

# ------------------------------------------------------------
# OSCOMP 评测入口：make all → kernel-rv kernel-la (+ disk 镜像)
# ------------------------------------------------------------

# 重建 os/.cargo/config.toml（评测机 clone 时过滤隐藏目录，构建前恢复链接脚本 rustflags）。
# - 有 os/cargo-vendor-config.toml（评测/镜像离线场景）：生成最小 config 并追加 vendored 源；
# - 或 .cargo/config.toml 缺失：生成最小 config；
# - 否则（本地开发，config 已存在）：保留现有 config，不破坏 runner/测试设置。
# 注意：不写 build-std——两架构预编译 std 已自带 weak mem*，写了反而拖慢构建并破坏 cargo test。
os-cargo-config:
	@if [ -f "$(OS_DIR)/cargo-vendor-config.toml" ] || [ ! -f "$(OS_DIR)/.cargo/config.toml" ]; then \
		echo "[OSCOMP] 重建 $(OS_DIR)/.cargo/config.toml"; \
		mkdir -p $(OS_DIR)/.cargo; \
		printf '%s\n' \
			'[target.riscv64gc-unknown-none-elf]' \
			'rustflags = ["-Clink-arg=-Tsrc/linker.ld", "-Cforce-frame-pointers=yes"]' \
			'' \
			'[target.loongarch64-unknown-none]' \
			'rustflags = ["-Clink-arg=-Tsrc/loongarch_linker.ld", "-Cforce-frame-pointers=yes"]' \
			> $(OS_DIR)/.cargo/config.toml; \
		if [ -f "$(OS_DIR)/cargo-vendor-config.toml" ]; then \
			printf '\n' >> $(OS_DIR)/.cargo/config.toml; \
			cat "$(OS_DIR)/cargo-vendor-config.toml" >> $(OS_DIR)/.cargo/config.toml; \
			echo "[OSCOMP] 已追加 vendored 源（离线构建）"; \
			echo "[OSCOMP] 重建 vendor 校验和（评测机过滤删除了 .cargo-checksum.json）"; \
			python3 scripts/restore_vendor_checksums.py $(OS_DIR); \
		fi; \
	else \
		echo "[OSCOMP] 保留现有 $(OS_DIR)/.cargo/config.toml（本地开发）"; \
	fi

all: os-cargo-config kernel-rv kernel-la disk.img disk-la.img
	@echo "[OSCOMP] 完成：kernel-rv kernel-la disk.img disk-la.img"

kernel-rv: os-cargo-config
	@echo "[OSCOMP] 构建 RISC-V 内核 (ELF, $(PROFILE_DIR)): kernel-rv"
	cd $(OS_DIR) && ARCH=riscv cargo build $(CARGO_PROFILE_FLAG) --target $(RV_TARGET) $(OSCOMP_FEATURE)
	cp -f $(OS_DIR)/target/$(RV_TARGET)/$(PROFILE_DIR)/$(OS_BIN) kernel-rv

kernel-la: os-cargo-config
	@echo "[OSCOMP] 构建 LoongArch 内核 (ELF, $(PROFILE_DIR)): kernel-la"
	cd $(OS_DIR) && ARCH=loongarch cargo build $(CARGO_PROFILE_FLAG) --target $(LA_TARGET) $(OSCOMP_FEATURE)
	cp -f $(OS_DIR)/target/$(LA_TARGET)/$(PROFILE_DIR)/$(OS_BIN) kernel-la

# rootfs 镜像由 os/build.rs 在对应内核构建时生成（os/fs-{arch}.img），此处拷到根目录。
disk.img: kernel-rv
	@echo "[OSCOMP] 产出 rootfs 镜像: disk.img"
	@test -f $(OS_DIR)/fs-riscv.img
	cp -f $(OS_DIR)/fs-riscv.img disk.img

disk-la.img: kernel-la
	@echo "[OSCOMP] 产出 rootfs 镜像: disk-la.img"
	@test -f $(OS_DIR)/fs-loongarch.img
	cp -f $(OS_DIR)/fs-loongarch.img disk-la.img

# ------------------------------------------------------------
# 本地复现评测：启动 QEMU，只挂单盘 rootfs（内含 /tests），
# 内核自动跑 musl 测试并主动关机；-no-reboot 让关机时 QEMU 退出。
# 设备型号对齐 os/qemu-run.sh（riscv: virtio-mmio）与 os/qemu-loongarch-run.sh（loongarch: pci）。
# ------------------------------------------------------------
run-oscomp-rv: kernel-rv disk.img
	@echo "[OSCOMP] 运行 RISC-V QEMU（单盘 rootfs + tests，自动跑测试并关机）"
	qemu-system-riscv64 -machine virt -kernel kernel-rv -m $(OSCOMP_RV_MEM) -nographic \
		-smp $(OSCOMP_RV_SMP) -bios default -no-reboot -rtc base=utc \
		-drive file=disk.img,if=none,format=raw,id=x0 \
		-device virtio-blk-device,drive=x0,bus=virtio-mmio-bus.0 \
		-device virtio-net-device,netdev=net -netdev user,id=net

run-oscomp-la: kernel-la disk-la.img
	@echo "[OSCOMP] 运行 LoongArch QEMU（单盘 rootfs + tests，自动跑测试并关机）"
	qemu-system-loongarch64 -machine virt -kernel kernel-la -m $(OSCOMP_LA_MEM) -nographic \
		-smp $(OSCOMP_LA_SMP) -no-reboot -rtc base=utc \
		-drive file=disk-la.img,if=none,format=raw,id=x0 \
		-device virtio-blk-pci,drive=x0 \
		-device virtio-net-pci,netdev=net0 \
		-netdev user,id=net0,hostfwd=tcp::5555-:5555,hostfwd=udp::5555-:5555

# 清理 OS 构建产物
clean:
	cd os && cargo clean

# 清理所有构建产物（包括 user）
clean-all: clean
	cd user && make clean

# 手动编译用户程序（通常不需要，build.rs 会自动处理）
build-user:
	cd user && make

# 手动打包镜像（通常不需要，build.rs 会自动处理）
pack-simple-fs: build-user
	@echo "Packing simple_fs..."
	python3 scripts/make_init_simple_fs.py user/bin os/simple_fs.img

# 检查镜像内容
inspect-simple-fs:
		@IMG=$$(find os/target -name "simple_fs.img" -type f -print -quit 2>/dev/null); \
	if [ -z "$$IMG" ]; then \
		echo "Error: simple_fs.img not found. Run 'make build' first." >&2; \
		exit 1; \
	else \
		python3 scripts/make_init_simple_fs.py --inspect "$$IMG"; \
	fi

# 帮助信息
help:
	@echo "ComixOS Makefile"
	@echo ""
	@echo "Usage: make [target] ARCH=[riscv|loongarch]"
	@echo ""
	@echo "Architectures:"
	@echo "  riscv      - RISC-V 64-bit (default)"
	@echo "  loongarch  - LoongArch 64-bit"
	@echo ""
	@echo "Targets:"
	@echo "  build      - Build the kernel"
	@echo "  run        - Run the kernel in QEMU"
	@echo "  gdb        - Run with GDB debugging"
	@echo "  clean      - Clean build artifacts"
	@echo "  help       - Show this help message"
	@echo ""
	@echo "Examples:"
	@echo "  make build                    # Build for RISC-V"
	@echo "  make build ARCH=loongarch     # Build for LoongArch"
	@echo "  make run ARCH=loongarch       # Run LoongArch kernel"
