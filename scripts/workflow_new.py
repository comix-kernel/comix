#!/usr/bin/env python3
"""Create a tool-agnostic AI workflow task from repository templates."""

from __future__ import annotations

import argparse
import re
import sys
from datetime import datetime, timezone
from pathlib import Path


SCRIPT_PATH = Path(__file__).resolve()
if SCRIPT_PATH.parent.parent.name == ".ai-workflow":
    WORKFLOW_ROOT = SCRIPT_PATH.parents[1]
    ROOT = WORKFLOW_ROOT.parent
else:
    ROOT = SCRIPT_PATH.parents[1]
    WORKFLOW_ROOT = ROOT / ".ai-workflow"
TEMPLATE_DIR = WORKFLOW_ROOT / "templates"
TASK_DIR = WORKFLOW_ROOT / "tasks"

FILES = [
    ("requirement.md", "01-requirement.md"),
    ("spec.md", "02-spec.md"),
    ("implementation-plan.md", "03-implementation-plan.md"),
    ("implementation-result.md", "04-implementation-result.md"),
    ("qa-report.md", "05-qa-report.md"),
    ("ai-review.md", "06-ai-review.md"),
    ("final-approval.md", "07-final-approval.md"),
    ("pr-description.md", "08-pr-description.md"),
    ("commit-message.txt", "commit-message.txt"),
    ("rejection.md", "rejection.md"),
    ("status.json", "status.json"),
]


def slugify(value: str) -> str:
    slug = re.sub(r"[^\w.-]+", "-", value.strip(), flags=re.UNICODE).strip("-")
    return slug.lower()


def render(template: str, task: str) -> str:
    created_at = datetime.now(timezone.utc).isoformat(timespec="seconds")
    return template.replace("{{TASK}}", task).replace("{{CREATED_AT}}", created_at)


def create_task(task: str, force: bool, dry_run: bool) -> Path:
    slug = slugify(task)
    if not slug:
        raise ValueError("TASK must contain at least one letter, digit, dot, underscore, or dash")

    dest_dir = TASK_DIR / slug
    if dest_dir.exists() and not force:
        raise FileExistsError(f"task already exists: {dest_dir}")

    for template_name, _ in FILES:
        template_path = TEMPLATE_DIR / template_name
        if not template_path.exists():
            raise FileNotFoundError(f"missing template: {template_path}")

    if dry_run:
        return dest_dir

    dest_dir.mkdir(parents=True, exist_ok=True)
    for template_name, output_name in FILES:
        template_path = TEMPLATE_DIR / template_name
        output_path = dest_dir / output_name
        if output_path.exists() and not force:
            continue
        output_path.write_text(render(template_path.read_text(encoding="utf-8"), task), encoding="utf-8")

    return dest_dir


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("task", help="task name, for example: getcwd-syscall")
    parser.add_argument("--force", action="store_true", help="overwrite existing generated files")
    parser.add_argument("--dry-run", action="store_true", help="validate templates and print the target directory")
    args = parser.parse_args()

    try:
        dest_dir = create_task(args.task, args.force, args.dry_run)
    except Exception as exc:
        print(f"workflow-new: {exc}", file=sys.stderr)
        return 1

    print(dest_dir.relative_to(ROOT))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
