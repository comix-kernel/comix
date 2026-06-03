#!/usr/bin/env python3
"""Print the next actionable workflow stage."""

from __future__ import annotations

import argparse
import sys

from workflow_lib import STAGE_BY_KEY, first_incomplete_stage, require_task, sync_json_from_markdown


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("task", help="task name")
    args = parser.parse_args()

    try:
        path = require_task(args.task)
        data = sync_json_from_markdown(path)
        stage_key = first_incomplete_stage(path)
    except Exception as exc:
        print(f"workflow-next: {exc}", file=sys.stderr)
        return 1

    if not stage_key:
        print("next: complete")
        return 0

    print(f"next: {stage_key} ({STAGE_BY_KEY[stage_key].label})")
    print(f"current_stage: {data.get('current_stage', stage_key)}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
