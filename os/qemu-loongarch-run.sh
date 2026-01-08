#!/bin/bash
# LoongArch64 QEMU 运行脚本（结构对齐 RISC-V 版本）

KERNEL=$1
MODE=${2:-run}

# 参数定义（对齐评测指令）
mem="4G"
smp="1"
arch="${ARCH:-loongarch}"
fs="fs-${arch}.img"
disk="disk-la.img"

# 创建空磁盘镜像（如果不存在）
if [ ! -f "$disk" ]; then
    dd if=/dev/zero of="$disk" bs=1M count=32 2>/dev/null
fi

QEMU_ARGS=(
    -machine virt
    -kernel "$KERNEL"
    -m "$mem"
    -nographic
    -smp "$smp"
    -no-reboot
)

# Virtio Block 设备 (fs.img)
if [ -f "$fs" ]; then
    QEMU_ARGS+=(-drive file="$fs",if=none,format=raw,id=x0)
    QEMU_ARGS+=(-device virtio-blk-pci,drive=x0)
elif [ -f "fs.img" ]; then
    fs="fs.img"
    QEMU_ARGS+=(-drive file="$fs",if=none,format=raw,id=x0)
    QEMU_ARGS+=(-device virtio-blk-pci,drive=x0)
else
    echo "Error: ${fs} not found!"
    echo "Please run 'cargo build' first to generate the filesystem image."
    exit 1
fi

# Virtio Network 设备
QEMU_ARGS+=(-device virtio-net-pci,netdev=net0)
QEMU_ARGS+=(-netdev user,id=net0,hostfwd=tcp::5555-:5555,hostfwd=udp::5555-:5555)

# RTC 设备 (基于 UTC 时间)
QEMU_ARGS+=(-rtc base=utc)

# 附加磁盘 (disk-la.img)
if [ -f "$disk" ]; then
    QEMU_ARGS+=(-drive file="$disk",if=none,format=raw,id=x1)
    QEMU_ARGS+=(-device virtio-blk-pci,drive=x1)
fi

case $MODE in
    run)
        qemu-system-loongarch64 "${QEMU_ARGS[@]}"
        ;;
    gdb)
        QEMU_ARGS+=(-s -S)
        qemu-system-loongarch64 "${QEMU_ARGS[@]}"
        ;;
    *)
        echo "Usage: $0 <kernel> [run|gdb]"
        exit 1
        ;;
esac
