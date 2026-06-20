#!/usr/bin/env bash
set -euo pipefail

usage() {
    echo "Usage: $0 <rootfs-ext4.img> <vfat.img> <output-disk.img>" >&2
}

if [ "$#" -ne 3 ]; then
    usage
    exit 2
fi

rootfs="$1"
vfat="$2"
out="$3"

sector_size="${CCYOS_SECTOR_SIZE:-512}"
align_sectors="${CCYOS_PART_ALIGN_SECTORS:-2048}"

if [ ! -f "$rootfs" ]; then
    echo "Error: rootfs image not found: $rootfs" >&2
    exit 1
fi

if [ ! -f "$vfat" ]; then
    echo "Error: VFAT image not found: $vfat" >&2
    exit 1
fi

root_bytes=$(stat -c%s "$rootfs")
vfat_bytes=$(stat -c%s "$vfat")
root_sectors=$(((root_bytes + sector_size - 1) / sector_size))
vfat_sectors=$(((vfat_bytes + sector_size - 1) / sector_size))

root_start=$align_sectors
root_end=$((root_start + root_sectors))
vfat_start=$((((root_end + align_sectors - 1) / align_sectors) * align_sectors))
vfat_end=$((vfat_start + vfat_sectors))
disk_sectors=$((vfat_end + align_sectors))

rm -f "$out"
truncate -s $((disk_sectors * sector_size)) "$out"

{
    echo "label: dos"
    echo "unit: sectors"
    echo "start=$root_start, size=$root_sectors, type=83"
    echo "start=$vfat_start, size=$vfat_sectors, type=0c"
} | sfdisk "$out"

dd if="$rootfs" of="$out" bs="$sector_size" seek="$root_start" conv=notrunc status=none
dd if="$vfat" of="$out" bs="$sector_size" seek="$vfat_start" conv=notrunc status=none

echo "[Disk] $out: vda1 ext4 start=$root_start sectors=$root_sectors, vda2 vfat start=$vfat_start sectors=$vfat_sectors"
