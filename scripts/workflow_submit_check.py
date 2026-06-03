#!/usr/bin/env python3
"""Check whether a workflow task is ready for commit or PR submission."""

from __future__ import annotations

import argparse
import sys

from workflow_lib import STAGES, markdown_stage_status, require_task, sync_json_from_markdown


PLACEHOLDERS = [
    "<scope>",
    "<summary",
    "待定",
    "通过 / 不通过 / 待定",
]


def has_meaningful_content(text: str) -> bool:
    stripped = text.strip()
    if not stripped:
        return False
    return not all(marker in stripped for marker in PLACEHOLDERS[:2])


def check_file(path, label: str) -> list[str]:
    if not path.exists():
        return [f"{label}: missing ({path.name})"]
    text = path.read_text(encoding="utf-8")
    issues = []
    for placeholder in PLACEHOLDERS:
        if placeholder in text:
            issues.append(f"{label}: contains placeholder `{placeholder}`")
    if not has_meaningful_content(text):
        issues.append(f"{label}: no meaningful content")
    return issues


def submit_check(task: str) -> int:
    try:
        path = require_task(task)
        sync_json_from_markdown(path)
    except Exception as exc:
        print(f"workflow-submit-check: {exc}", file=sys.stderr)
        return 1

    issues = []
    for stage in STAGES:
        status = markdown_stage_status(path, stage.key)
        if status != "approved":
            issues.append(f"{stage.label}: expected approved, got {status}")

    qa_text = (path / "05-qa-report.md").read_text(encoding="utf-8") if (path / "05-qa-report.md").exists() else ""
    if "## 验证记录" not in qa_text:
        issues.append("QA: missing verification record; run `make workflow-verify TASK=<name>` or document why it cannot run")

    issues.extend(check_file(path / "08-pr-description.md", "PR 描述"))
    issues.extend(check_file(path / "commit-message.txt", "Commit message"))

    if issues:
        print("Result: not ready")
        for issue in issues:
            print(f"- {issue}")
        return 1

    print("Result: ready for submission")
    return 0


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("task", help="task name")
    args = parser.parse_args()
    return submit_check(args.task)


if __name__ == "__main__":
    raise SystemExit(main())
