#!/usr/bin/env python3
"""
mdBook 链接重写脚本：将 /os/src/... 路径转换为 GitHub URL

使用方式：
  python3 scripts/rewrite_links.py document/
"""

import re
import sys
from pathlib import Path

# GitHub 仓库配置
GITHUB_REPO = "https://github.com/comix-kernel/comix"
GITHUB_BRANCH = "main"

# 链接模式：支持以下格式
# 1. [text](/os/src/path/file.rs)
# 2. [text](/os/src/path/file.rs:line)
# 3. [text](/os/src/path/file.rs:start-end)
LINK_PATTERN = r'\[([^\]]+)\]\(/os/src/([^\)]+)\)'


def convert_link(match):
    """将单个链接转换为 GitHub URL"""
    text = match.group(1)
    path = match.group(2)

    # 处理行号：/os/src/mm/address.rs:12 -> /os/src/mm/address.rs + #L12
    line_anchor = ""
    if ":" in path:
        file_path, line_spec = path.rsplit(":", 1)
        # 支持单行 (L12) 和行范围 (L12-L34)
        if "-" in line_spec:
            start_line, end_line = line_spec.split("-")
            line_anchor = f"#L{start_line}-L{end_line}"
        else:
            line_anchor = f"#L{line_spec}"
    else:
        file_path = path

    # 构造 GitHub URL
    github_url = f"{GITHUB_REPO}/blob/{GITHUB_BRANCH}/os/src/{file_path}{line_anchor}"

    return f"[{text}]({github_url})"


def process_file(file_path):
    """处理单个 Markdown 文件"""
    with open(file_path, 'r', encoding='utf-8') as f:
        content = f.read()

    # 计数：用于验证处理结果
    original_count = len(re.findall(LINK_PATTERN, content))

    # 替换链接
    new_content = re.sub(LINK_PATTERN, convert_link, content)

    # 检查是否有变化
    if content != new_content:
        with open(file_path, 'w', encoding='utf-8') as f:
            f.write(new_content)
        return original_count, True

    return 0, False


def main():
    if len(sys.argv) < 2:
        print("用法: python3 scripts/rewrite_links.py <document_dir>")
        sys.exit(1)

    doc_dir = Path(sys.argv[1])

    if not doc_dir.exists():
        print(f"错误: 目录不存在 {doc_dir}")
        sys.exit(1)

    total_links = 0
    modified_files = 0

    # 处理所有 Markdown 文件
    for md_file in doc_dir.rglob("*.md"):
        link_count, was_modified = process_file(md_file)
        if was_modified:
            total_links += link_count
            modified_files += 1
            print(f"✓ {md_file.relative_to(doc_dir)} - {link_count} 个链接")

    print(f"\n总结: 修改 {modified_files} 个文件, 转换 {total_links} 个链接")

    if modified_files > 0:
        print("链接已转换为 GitHub URL 格式")
    else:
        print("未发现需要转换的链接")


if __name__ == "__main__":
    main()
