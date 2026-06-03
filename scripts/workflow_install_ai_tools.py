#!/usr/bin/env python3
"""Copy AI workflow command and skill files into tool-specific local folders."""

from __future__ import annotations

import argparse
import shutil
import sys
from pathlib import Path


SCRIPT_PATH = Path(__file__).resolve()
if SCRIPT_PATH.parent.parent.name == ".ai-workflow":
    WORKFLOW_ROOT = SCRIPT_PATH.parents[1]
    ROOT = WORKFLOW_ROOT.parent
else:
    ROOT = SCRIPT_PATH.parents[1]
    WORKFLOW_ROOT = ROOT / ".ai-workflow"


def copy_tree_files(src: Path, dst: Path, pattern: str = "*") -> int:
    if not src.exists():
        raise FileNotFoundError(f"missing source: {src}")
    dst.mkdir(parents=True, exist_ok=True)
    count = 0
    for path in src.glob(pattern):
        if path.is_file():
            shutil.copy2(path, dst / path.name)
            count += 1
    return count


def install_claude() -> None:
    count = copy_tree_files(WORKFLOW_ROOT / "commands", ROOT / ".claude" / "commands", "*.md")
    print(f"installed Claude commands: {count}")


def install_codex() -> None:
    src = WORKFLOW_ROOT / "skills" / "ai-workflow"
    dst = ROOT / ".codex" / "skills" / "ai-workflow"
    if dst.exists():
        shutil.rmtree(dst)
    shutil.copytree(src, dst)
    print(f"installed Codex skill: {dst.relative_to(ROOT)}")


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--claude", action="store_true", help="copy commands to .claude/commands")
    parser.add_argument("--codex", action="store_true", help="copy skill to .codex/skills/ai-workflow")
    parser.add_argument("--all", action="store_true", help="install all supported local integrations")
    args = parser.parse_args()

    try:
        if args.all or args.claude:
            install_claude()
        if args.all or args.codex:
            install_codex()
        if not (args.all or args.claude or args.codex):
            parser.print_help()
    except Exception as exc:
        print(f"workflow-install-ai-tools: {exc}", file=sys.stderr)
        return 1

    return 0


if __name__ == "__main__":
    raise SystemExit(main())
