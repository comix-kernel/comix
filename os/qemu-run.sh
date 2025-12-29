#!/bin/bash
ELF_FILE="$1"
BIN_FILE="${ELF_FILE%.*}.bin"

# 参数定义
os_file="$BIN_FILE"
mem="4G"
smp="${SMP:-1}"  # 从环境变量读取，默认为 1
fs="fs.img"
disk="disk.img"

# 1. 转换为纯二进制
rust-objcopy --strip-all "$ELF_FILE" -O binary "$BIN_FILE"

# 2. 检查 fs.img (1GB Ext4 文件系统)
# 镜像应该由 build.rs 在编译时创建
if [ ! -f "fs.img" ]; then
    echo "Error: fs.img not found!"
    echo "Please run 'cargo build' first to generate the filesystem image."
    exit 1
fi

echo "Using existing fs.img (1GB Ext4 filesystem)"

# 3. 运行 QEMU
QEMU_ARGS="-machine virt \
            -kernel $os_file \
            -display none \
            -smp cpus=$smp,maxcpus=$smp \
            -bios default \
            -no-reboot"

# 串口设备
QEMU_ARGS="$QEMU_ARGS -serial stdio"

# RTC 设备 (基于 UTC 时间)
QEMU_ARGS="$QEMU_ARGS -rtc base=utc"

# Virtio Block 设备
QEMU_ARGS="$QEMU_ARGS -drive file=$fs,if=none,format=raw,id=x0"
QEMU_ARGS="$QEMU_ARGS -device virtio-blk-device,drive=x0,bus=virtio-mmio-bus.0"

# Virtio Network 设备
QEMU_ARGS="$QEMU_ARGS -device virtio-net-device,netdev=net"
QEMU_ARGS="$QEMU_ARGS -netdev user,id=net,hostfwd=tcp::8080-:80"

# GDB 调试模式
if [ "$2" == "gdb" ]; then
    echo "Starting QEMU in GDB debug mode on port 1234."
    QEMU_ARGS="$QEMU_ARGS -S -gdb tcp::1234"
else
    echo "Starting QEMU in normal run mode."
fi

qemu-system-riscv64 $QEMU_ARGS