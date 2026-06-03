#!/usr/bin/env python3
"""Check approval status and stage order for a tool-agnostic AI workflow task."""

from __future__ import annotations

import argparse
import sys

from workflow_lib import ROOT, STAGES, classify_status, extract_status, require_task, sync_json_from_markdown


def check_task(task: str) -> int:
    try:
        task_path = require_task(task)
    except Exception as exc:
        print(f"workflow-check: {exc}", file=sys.stderr)
        return 1

    failed = False
    blocked = False
    seen_incomplete = False
    data = sync_json_from_markdown(task_path)
    print(f"Task: {task_path.relative_to(ROOT)}")
    print(f"Current stage: {data.get('current_stage', 'unknown')}")
    for stage in STAGES:
        path = task_path / stage.file
        if not path.exists():
            print(f"- {stage.label}: missing ({stage.file})")
            failed = True
            seen_incomplete = True
            continue
        status = extract_status(path.read_text(encoding="utf-8"))
        kind = classify_status(status)
        order_note = ""
        if kind == "approved" and seen_incomplete:
            order_note = " [order violation: previous stage incomplete]"
            failed = True
        print(f"- {stage.label}: {kind} ({status}){order_note}")
        if kind == "rejected":
            blocked = True
        if kind in {"missing", "pending", "rejected"}:
            failed = True
            seen_incomplete = True

    rejection_path = task_path / "rejection.md"
    if rejection_path.exists():
        rejection_text = rejection_path.read_text(encoding="utf-8")
        if "待定" not in rejection_text:
            print("- 驳回记录: present")

    if blocked:
        print("Result: blocked by rejection")
        return 2
    if failed:
        print("Result: incomplete")
        return 1
    print("Result: complete")
    return 0


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("task", help="task name used by workflow_new.py")
    args = parser.parse_args()
    return check_task(args.task)


if __name__ == "__main__":
    raise SystemExit(main())
