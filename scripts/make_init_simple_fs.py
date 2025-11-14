#!/usr/bin/env python3
"""
Simple_fs 镜像打包工具

将用户程序打包成简单的块设备镜像格式，供 RamDisk + SimpleFS 使用。

镜像格式：
- 镜像头: "RAMDISK\0" (8字节) + 文件数量 (4字节) + 保留 (4字节)
- 文件头: 魔数 (4) + 名称长度 (4) + 数据长度 (4) + 文件类型 (4) + 权限 (4) + padding (12)
- 文件名: UTF-8 编码，4字节对齐
- 文件数据: 原始数据，512字节对齐
"""

import struct
import sys
import argparse
from pathlib import Path
from typing import List, Tuple

# 常量定义
MAGIC = b"RAMDISK\0"
FILE_MAGIC = 0x46494C45  # "FILE"
BLOCK_SIZE = 512

# 文件类型
FILE_TYPE_FILE = 0
FILE_TYPE_DIR = 1


def align_to(size: int, alignment: int) -> int:
    """将大小对齐到指定边界"""
    return (size + alignment - 1) // alignment * alignment


def pack_file(path: Path, base: Path) -> bytes:
    """
    打包单个文件或目录

    Args:
        path: 文件/目录的绝对路径
        base: 基准目录路径，用于计算相对路径

    Returns:
        打包后的字节数据，包括文件头、文件名、文件数据
    """
    # 计算相对路径
    try:
        rel_path = path.relative_to(base)
    except ValueError:
        print(f"Warning: {path} is not relative to {base}, skipping")
        return b""

    # 跳过隐藏文件和特殊文件
    if rel_path.name.startswith('.') or rel_path.name in ['Cargo.lock', 'Cargo.toml']:
        return b""

    name_str = str(rel_path).replace('\\', '/')  # Windows 兼容
    name_bytes = name_str.encode('utf-8')

    # 确定文件类型和数据
    if path.is_file():
        data = path.read_bytes()
        file_type = FILE_TYPE_FILE
        mode = 0o644
    elif path.is_dir():
        data = b""
        file_type = FILE_TYPE_DIR
        mode = 0o755
    else:
        # 跳过符号链接等特殊文件
        return b""

    # 构建文件头 (32字节)
    header = struct.pack(
        '<5I3I',  # 小端序，5个uint32 + 3个padding
        FILE_MAGIC,
        len(name_bytes),
        len(data),
        file_type,
        mode,
        0, 0, 0  # padding
    )

    # 名称对齐到4字节
    name_aligned = name_bytes + b'\0' * (align_to(len(name_bytes), 4) - len(name_bytes))

    # 数据对齐到块大小
    data_aligned = data + b'\0' * (align_to(len(data), BLOCK_SIZE) - len(data))

    return header + name_aligned + data_aligned


def collect_files(src_dir: Path) -> List[Path]:
    """
    递归收集目录中的所有文件和目录

    Args:
        src_dir: 源目录路径

    Returns:
        文件路径列表（已排序）
    """
    if not src_dir.exists():
        return []

    items = []

    # 首先添加所有目录（深度优先）
    for item in sorted(src_dir.rglob('*')):
        if item.is_dir():
            items.append(item)

    # 然后添加所有文件
    for item in sorted(src_dir.rglob('*')):
        if item.is_file():
            # 跳过不需要的文件
            if item.suffix in ['.d', '.o', '.a', '.rlib']:
                continue
            if item.name in ['Cargo.lock', '.gitignore']:
                continue
            items.append(item)

    return items


def make_simple_fs(src_dir: Path, output: Path, verbose: bool = False) -> Tuple[int, int]:
    """
    生成 simple_fs 镜像

    Args:
        src_dir: 源目录路径
        output: 输出镜像路径
        verbose: 是否显示详细信息

    Returns:
        (文件数量, 镜像大小)
    """
    if not src_dir.exists():
        print(f"Warning: Source directory {src_dir} does not exist, creating empty image")
        # 创建空镜像
        header = MAGIC + struct.pack('<II', 0, 0)
        header += b'\0' * (16 - len(header))
        output.write_bytes(header)
        return 0, len(header)

    # 收集所有文件
    files = collect_files(src_dir)

    if verbose:
        print(f"Collecting files from {src_dir}...")
        print(f"Found {len(files)} items")

    # 打包所有文件
    packed_files = []
    for item in files:
        packed = pack_file(item, src_dir)
        if packed:
            packed_files.append(packed)
            if verbose:
                rel_path = item.relative_to(src_dir)
                file_type = "DIR " if item.is_dir() else "FILE"
                size = 0 if item.is_dir() else item.stat().st_size
                print(f"  [{file_type}] {rel_path} ({size} bytes)")

    # 构建镜像头 (16字节)
    header = MAGIC + struct.pack('<II', len(packed_files), 0)
    header += b'\0' * (16 - len(header))

    # 写入镜像
    output.parent.mkdir(parents=True, exist_ok=True)
    with open(output, 'wb') as f:
        f.write(header)
        for file_data in packed_files:
            f.write(file_data)

        # 填充镜像到块边界，确保镜像大小是 BLOCK_SIZE 的整数倍
        current_size = f.tell()
        aligned_size = align_to(current_size, BLOCK_SIZE)
        padding = aligned_size - current_size
        if padding > 0:
            f.write(b'\0' * padding)

    total_size = output.stat().st_size

    return len(packed_files), total_size


def inspect_simple_fs(img_path: Path):
    """
    检查 simple_fs 镜像内容（用于调试）

    Args:
        img_path: 镜像文件路径
    """
    with open(img_path, 'rb') as f:
        # 读取头部
        magic = f.read(8)
        if magic != MAGIC:
            print(f"Error: Invalid magic: {magic}")
            return

        file_count = struct.unpack('<I', f.read(4))[0]
        f.read(4)  # reserved

        print(f"Simple_fs Image: {img_path}")
        print(f"  Total files: {file_count}")
        print()

        # 读取每个文件
        for i in range(file_count):
            header = f.read(32)
            magic, name_len, data_len, file_type, mode = struct.unpack('<5I', header[:20])

            if magic != FILE_MAGIC:
                print(f"Error: Invalid file magic at entry {i}")
                break

            name_aligned = align_to(name_len, 4)
            data_aligned = align_to(data_len, BLOCK_SIZE)

            name = f.read(name_aligned)[:name_len].decode('utf-8')
            f.seek(data_aligned, 1)  # 跳过数据

            type_str = "FILE" if file_type == FILE_TYPE_FILE else "DIR "
            print(f"  [{type_str}] {name}")
            print(f"      Size: {data_len} bytes")
            print(f"      Mode: {oct(mode)}")


def main():
    parser = argparse.ArgumentParser(
        description='Pack files into simple_fs image for RamDisk',
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog="""
Examples:
  # Pack user programs
  %(prog)s user/bin os/simple_fs.img

  # Pack with verbose output
  %(prog)s -v user/bin os/simple_fs.img

  # Inspect existing image
  %(prog)s --inspect os/simple_fs.img
        """
    )

    parser.add_argument('src_dir', nargs='?', type=Path,
                        help='Source directory to pack')
    parser.add_argument('output', nargs='?', type=Path,
                        help='Output image file path')
    parser.add_argument('-v', '--verbose', action='store_true',
                        help='Show verbose output')
    parser.add_argument('-i', '--inspect', metavar='IMAGE', type=Path,
                        help='Inspect an existing simple_fs image')

    args = parser.parse_args()

    # 检查模式
    if args.inspect:
        if not args.inspect.exists():
            print(f"Error: Image file {args.inspect} does not exist")
            return 1
        inspect_simple_fs(args.inspect)
        return 0

    # 打包模式
    if not args.src_dir or not args.output:
        parser.print_help()
        return 1

    try:
        file_count, total_size = make_simple_fs(args.src_dir, args.output, args.verbose)
        print(f"✓ Created {args.output}: {file_count} files, {total_size} bytes ({total_size // 1024} KB)")
        return 0
    except Exception as e:
        print(f"Error: {e}")
        import traceback
        traceback.print_exc()
        return 1


if __name__ == '__main__':
    sys.exit(main())
