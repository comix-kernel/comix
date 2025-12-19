#!/bin/bash
# LoongArch64 QEMU 运行脚本

KERNEL=$1
MODE=${2:-run}
SMP=1

# 简单的文件系统镜像（如果存在）
FS_IMG="target/loongarch64-unknown-none/debug/simple_fs.img"
DISK_IMG="disk-la.img"

# 创建空磁盘镜像（如果不存在）
if [ ! -f "$DISK_IMG" ]; then
    dd if=/dev/zero of="$DISK_IMG" bs=1M count=32 2>/dev/null
fi

QEMU_ARGS=(
    -machine virt
    -smp "$SMP"
    -nographic
    -kernel "$KERNEL"
    -no-reboot
    -rtc base=utc
)

# 如果文件系统镜像存在，添加块设备
if [ -f "$FS_IMG" ]; then
    QEMU_ARGS+=(
        -drive file="$FS_IMG",if=none,format=raw,id=x0
        -device virtio-blk-pci,drive=x0,bus=virtio-mmio-bus.0
    )
fi

# 添加网络设备 (暂时禁用，避免端口冲突)
# QEMU_ARGS+=(
#     -device virtio-net-pci,netdev=net0
#     -netdev user,id=net0,hostfwd=tcp::5555-:5555,hostfwd=udp::5555-:5555
# )

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
