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
vfat="vfat.img"

QEMU_ARGS=(
    -machine virt
    -kernel "$KERNEL"
    -m "$mem"
    -nographic
    -smp "$smp"
    -no-reboot
)

if [ -f "$fs" ]; then
    :
elif [ -f "fs.img" ]; then
    fs="fs.img"
else
    echo "Error: ${fs} not found!"
    echo "Please run 'cargo build' first to generate the filesystem image."
    exit 1
fi

if [ ! -f "$vfat" ]; then
    echo "Creating ${vfat} (64MiB FAT32/VFAT partition image)"
    rm -f "$vfat"
    truncate -s 64M "$vfat"
    mkfs.vfat -F 32 -n CCYOSVFAT "$vfat"
fi

echo "Assembling ${disk} from ${fs} and ${vfat}"
../scripts/assemble_partitioned_disk.sh "$fs" "$vfat" "$disk"

# Virtio Block 设备（MBR 分区盘：vda1 rootfs，vda2 VFAT）
QEMU_ARGS+=(-drive file="$disk",if=none,format=raw,id=x0)
QEMU_ARGS+=(-device virtio-blk-pci,drive=x0)

# Virtio Network 设备
QEMU_ARGS+=(-device virtio-net-pci,netdev=net0)
QEMU_ARGS+=(-netdev user,id=net0,hostfwd=tcp::5555-:5555,hostfwd=udp::5555-:5555)

# RTC 设备 (基于 UTC 时间)
QEMU_ARGS+=(-rtc base=utc)

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
