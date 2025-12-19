#!/bin/bash
# LoongArch64 QEMU 运行脚本

KERNEL=$1
MODE=${2:-run}

QEMU_ARGS=(
    -machine virt
    -m 128M
    -nographic
    -bios none
    -kernel "$KERNEL"
)

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
