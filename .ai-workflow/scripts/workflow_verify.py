#!/usr/bin/env python3
"""Run workflow verification commands and optionally append a QA report entry."""

from __future__ import annotations

import argparse
import subprocess
import sys

from workflow_lib import ROOT, now, require_task


COMMANDS = [
    (["cargo", "fmt"], ROOT / "os"),
    (["make", "build"], ROOT),
    (["make", "test"], ROOT / "os"),
]


def output_tail(output: str, limit: int = 80) -> str:
    lines = output.splitlines()
    return "\n".join(lines[-limit:])


def append_qa(task: str, results: list[tuple[list[str], int, str]]) -> None:
    path = require_task(task)
    qa_path = path / "05-qa-report.md"
    blocks = [
        "",
        "## 验证记录",
        "",
        f"时间：{now()}",
        "",
    ]
    all_passed = True
    for command, code, output in results:
        all_passed = all_passed and code == 0
        blocks.extend(
            [
                f"### `{' '.join(command)}`",
                "",
                f"退出码：{code}",
                "",
                "```text",
                output_tail(output),
                "```",
                "",
            ]
        )
    blocks.extend(["结论：" + ("通过" if all_passed else "不通过"), ""])
    with qa_path.open("a", encoding="utf-8") as file:
        file.write("\n".join(blocks))


def run_commands(dry_run: bool) -> list[tuple[list[str], int, str]]:
    results = []
    for command, cwd in COMMANDS:
        print(f"+ ({cwd.relative_to(ROOT) if cwd != ROOT else '.'}) {' '.join(command)}")
        if dry_run:
            results.append((command, 0, "dry-run"))
            continue
        completed = subprocess.run(
            command,
            cwd=cwd,
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            check=False,
        )
        print(output_tail(completed.stdout, limit=30))
        results.append((command, completed.returncode, completed.stdout))
        if completed.returncode != 0:
            break
    return results


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--task", default="", help="task name; when set, append results to QA report")
    parser.add_argument("--dry-run", action="store_true", help="print commands without executing them")
    args = parser.parse_args()

    try:
        results = run_commands(args.dry_run)
        if args.task:
            append_qa(args.task, results)
    except Exception as exc:
        print(f"workflow-verify: {exc}", file=sys.stderr)
        return 1

    return 0 if all(code == 0 for _, code, _ in results) else 1


if __name__ == "__main__":
    raise SystemExit(main())
