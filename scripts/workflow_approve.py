#!/usr/bin/env python3
"""Approve one workflow stage and advance the task status."""

from __future__ import annotations

import argparse
import sys

from workflow_lib import (
    STAGE_BY_KEY,
    first_incomplete_stage,
    normalize_stage,
    read_status_json,
    require_task,
    set_stage_approval,
    sync_json_from_markdown,
    write_status_json,
)


def approve(task: str, stage: str, approver: str, note: str) -> int:
    try:
        path = require_task(task)
        stage_key = normalize_stage(stage)
        current = first_incomplete_stage(path)
        if current and current != stage_key:
            print(
                f"workflow-approve: cannot approve {stage_key}; current incomplete stage is {current}",
                file=sys.stderr,
            )
            return 2

        set_stage_approval(path, stage_key, "批准", approver, note)
        data = sync_json_from_markdown(path)
        data = read_status_json(path)
        data["stages"][stage_key] = "approved"
        data["current_stage"] = first_incomplete_stage(path) or "complete"
        write_status_json(path, data)
    except Exception as exc:
        print(f"workflow-approve: {exc}", file=sys.stderr)
        return 1

    print(f"approved: {STAGE_BY_KEY[stage_key].label}")
    print(f"current_stage: {data['current_stage']}")
    return 0


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("task", help="task name")
    parser.add_argument("stage", help="stage key or label")
    parser.add_argument("--by", default="human", help="approver name")
    parser.add_argument("--note", default="", help="approval note")
    args = parser.parse_args()
    return approve(args.task, args.stage, args.by, args.note)


if __name__ == "__main__":
    raise SystemExit(main())
