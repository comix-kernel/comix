#!/bin/bash
ELF_FILE="$1"
BIN_FILE="${ELF_FILE%.*}.bin"

# 1. 转换为纯二进制
rust-objcopy --strip-all "$ELF_FILE" -O binary "$BIN_FILE"

# 2. 运行 QEMU
qemu-system-riscv64 -machine virt -display none -serial stdio -bios ../bootloader/rustsbi-qemu.bin -device loader,file="$BIN_FILE",addr=0x80200000