#!/usr/bin/env python3
"""List repository workflow tasks and their current stages."""

from __future__ import annotations

import argparse

from workflow_lib import STAGE_BY_KEY, TASK_DIR, read_status_json, sync_json_from_markdown


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--sync", action="store_true", help="sync status.json from Markdown before listing")
    args = parser.parse_args()

    if not TASK_DIR.exists():
        print("No workflow tasks.")
        return 0

    rows = []
    for path in sorted(TASK_DIR.iterdir()):
        if not path.is_dir():
            continue
        data = sync_json_from_markdown(path) if args.sync else read_status_json(path)
        current = data.get("current_stage", "unknown")
        label = STAGE_BY_KEY[current].label if current in STAGE_BY_KEY else current
        rows.append((path.name, current, label))

    if not rows:
        print("No workflow tasks.")
        return 0

    print(f"{'task':<32} {'current_stage':<24} label")
    print(f"{'-' * 32} {'-' * 24} {'-' * 16}")
    for name, current, label in rows:
        print(f"{name:<32} {current:<24} {label}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
