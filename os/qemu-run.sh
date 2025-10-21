#!/bin/bash
ELF_FILE="$1"
BIN_FILE="${ELF_FILE%.*}.bin"

# 1. 转换为纯二进制
rust-objcopy --strip-all "$ELF_FILE" -O binary "$BIN_FILE"

# 2. 运行 QEMU
QEMU_ARGS="-machine virt \
            -display none \
            -serial stdio \
            -bios ../bootloader/rustsbi-qemu.bin \
            -device loader,file=$BIN_FILE,addr=0x80200000"

if [ "$2" == "gdb" ]; then
    echo "Starting QEMU in GDB debug mode on port 1234."
    QEMU_ARGS="$QEMU_ARGS -S -gdb tcp::1234"
else
    echo "Starting QEMU in normal run mode."
fi

qemu-system-riscv64 $QEMU_ARGS