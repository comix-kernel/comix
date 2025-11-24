#!/bin/bash
ELF_FILE="$1"
BIN_FILE="${ELF_FILE%.*}.bin"

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
            -display none \
            -bios ../bootloader/rustsbi-qemu.bin \
            -device loader,file=$BIN_FILE,addr=0x80200000"

# 串口设备 (UART16550 @ 0x10000000)
QEMU_ARGS="$QEMU_ARGS -serial stdio"

# Virtio Block 设备 (@ 0x10001000)
QEMU_ARGS="$QEMU_ARGS -drive file=fs.img,if=none,format=raw,id=x0"
QEMU_ARGS="$QEMU_ARGS -device virtio-blk-device,drive=x0"

# Virtio Network 设备 (@ 0x10002000)
QEMU_ARGS="$QEMU_ARGS -device virtio-net-device,netdev=net0"
QEMU_ARGS="$QEMU_ARGS -netdev user,id=net0,hostfwd=tcp::8080-:80"

# GDB 调试模式
if [ "$2" == "gdb" ]; then
    echo "Starting QEMU in GDB debug mode on port 1234."
    QEMU_ARGS="$QEMU_ARGS -S -gdb tcp::1234"
else
    echo "Starting QEMU in normal run mode."
fi

echo "MMIO devices enabled:"
echo "  - UART16550      @ 0x10000000"
echo "  - Virtio Block   @ 0x10001000 (fs.img - 1GB Ext4)"
echo "  - Virtio Network @ 0x10002000"
echo "  - PLIC           @ 0x0C000000 (virt machine built-in)"
echo "  - TEST/RTC       @ 0x00100000 (virt machine built-in)"

qemu-system-riscv64 $QEMU_ARGS