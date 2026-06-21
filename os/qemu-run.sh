#!/bin/bash
ELF_FILE="$1"
BIN_FILE="${ELF_FILE%.*}.bin"

# 参数定义
os_file="$BIN_FILE"
mem="4G"
smp="${SMP:-1}"  # 从环境变量读取，默认为 1
arch="${ARCH:-riscv}"
fs="fs-${arch}.img"
disk="disk.img"
vfat="vfat.img"
test_img="../sdcard-rv.img"

# 1. 转换为纯二进制
rust-objcopy --strip-all "$ELF_FILE" -O binary "$BIN_FILE"

# 2. 检查 rootfs 中间产物，并组装运行用 MBR 分区盘
if [ ! -f "$fs" ]; then
    if [ -f "fs.img" ]; then
        fs="fs.img"
    else
        echo "Error: ${fs} not found!"
        echo "Please run 'cargo build' first to generate the filesystem image."
        exit 1
    fi
fi

if [ ! -f "$vfat" ]; then
    echo "Creating ${vfat} (64MiB FAT32/VFAT partition image)"
    rm -f "$vfat"
    truncate -s 64M "$vfat"
    mkfs.vfat -F 32 -n CCYOSVFAT "$vfat"
fi

echo "Assembling ${disk} from ${fs} and ${vfat}"
../scripts/assemble_partitioned_disk.sh "$fs" "$vfat" "$disk"

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

if [ ! -f "$test_img" ]; then
    echo "Error: official test image ${test_img} not found!" >&2
    exit 1
fi

# Virtio Block 设备（MBR 分区盘：vda1=rootfs，vda2=VFAT）
# RISC-V virtio-mmio 设备树按高地址先探测；我们的分区盘放 bus.1 才会注册为 /dev/vda。
QEMU_ARGS="$QEMU_ARGS -drive file=$disk,if=none,format=raw,id=x0"
QEMU_ARGS="$QEMU_ARGS -device virtio-blk-device,drive=x0,bus=virtio-mmio-bus.1"

# 官方测试盘是额外裸 ext4 盘，固定注册为 /dev/vdb 后由 rcS 挂载到 /tests。
echo "Attaching official test image ${test_img} as vdb"
QEMU_ARGS="$QEMU_ARGS -drive file=$test_img,if=none,format=raw,id=test0"
QEMU_ARGS="$QEMU_ARGS -device virtio-blk-device,drive=test0,bus=virtio-mmio-bus.0"

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
