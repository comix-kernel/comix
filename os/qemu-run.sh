#!/bin/bash
ELF_FILE="$1"
BIN_FILE="${ELF_FILE%.*}.bin"

# 1. 转换为纯二进制
rust-objcopy --strip-all "$ELF_FILE" -O binary "$BIN_FILE"

# 2. 创建或检查 fs.img (128MB Ext4 文件系统)
if [ ! -f "fs.img" ]; then
    echo "Creating 128MB Ext4 filesystem image..."
    # 创建空白镜像
    dd if=/dev/zero of=fs.img bs=1M count=128 2>/dev/null
    # 格式化为 Ext4，参数参考 GitHub feat/fs/ext4 分支
    mkfs.ext4 -F -b 4096 -m 0 fs.img >/dev/null 2>&1
    echo "fs.img created successfully (128MB Ext4, 4KB blocks, 0% reserved)"
fi

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
echo "  - Virtio Block   @ 0x10001000 (fs.img - 128MB Ext4)"
echo "  - Virtio Network @ 0x10002000"
echo "  - PLIC           @ 0x0C000000 (virt machine built-in)"
echo "  - TEST/RTC       @ 0x00100000 (virt machine built-in)"

qemu-system-riscv64 $QEMU_ARGS