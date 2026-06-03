#!/usr/bin/env python3
"""Validate workflow templates for required files and approval fields."""

from __future__ import annotations

import json
import sys

from workflow_lib import STAGES, TEMPLATE_DIR


REQUIRED_TEMPLATES = {
    "requirement.md": True,
    "spec.md": True,
    "implementation-plan.md": True,
    "implementation-result.md": True,
    "qa-report.md": True,
    "ai-review.md": True,
    "final-approval.md": True,
    "rejection.md": False,
    "pr-description.md": False,
    "commit-message.txt": False,
    "status.json": False,
}


def main() -> int:
    issues = []
    for filename, requires_approval in REQUIRED_TEMPLATES.items():
        path = TEMPLATE_DIR / filename
        if not path.exists():
            issues.append(f"missing template: {filename}")
            continue
        if requires_approval:
            text = path.read_text(encoding="utf-8")
            if "## 人工审批" not in text:
                issues.append(f"{filename}: missing `## 人工审批`")
            if "状态：待定" not in text:
                issues.append(f"{filename}: missing `状态：待定`")

    status_path = TEMPLATE_DIR / "status.json"
    if status_path.exists():
        try:
            data = json.loads(status_path.read_text(encoding="utf-8"))
            stages = data.get("stages", {})
            for stage in STAGES:
                if stage.key not in stages:
                    issues.append(f"status.json: missing stage `{stage.key}`")
        except json.JSONDecodeError as exc:
            issues.append(f"status.json: invalid JSON: {exc}")

    if issues:
        print("Result: template check failed")
        for issue in issues:
            print(f"- {issue}")
        return 1

    print("Result: templates ok")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
