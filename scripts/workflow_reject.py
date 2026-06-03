#!/usr/bin/env python3
"""Reject one workflow stage and record rollback guidance."""

from __future__ import annotations

import argparse
import sys

from workflow_lib import (
    STAGE_BY_KEY,
    infer_rollback_stage,
    normalize_stage,
    read_status_json,
    replace_section_body,
    require_task,
    set_stage_approval,
    write_status_json,
)


def reject(
    task: str,
    stage: str,
    rejection_type: str,
    reason: str,
    must_fix: str,
    suggestion: str,
    allow_ai: str,
    rollback: str | None,
    approver: str,
) -> int:
    try:
        path = require_task(task)
        stage_key = normalize_stage(stage)
        rollback_key = infer_rollback_stage(rejection_type, rollback)

        set_stage_approval(path, stage_key, "驳回", approver, reason)
        if rollback_key != stage_key:
            set_stage_approval(path, rollback_key, "待定", approver, f"由 {STAGE_BY_KEY[stage_key].label} 驳回回退。")
        data = read_status_json(path)
        stages = data.setdefault("stages", {})
        stages[stage_key] = "rejected"
        stages[rollback_key] = "pending"
        data["current_stage"] = rollback_key
        write_status_json(path, data)

        rejection_path = path / "rejection.md"
        text = rejection_path.read_text(encoding="utf-8")
        text = replace_section_body(text, "## 驳回阶段", STAGE_BY_KEY[stage_key].label)
        text = replace_section_body(text, "## 驳回类型", rejection_type)
        text = replace_section_body(text, "## 驳回原因", reason)
        text = replace_section_body(text, "## 必须修改", f"- {must_fix}" if must_fix else "- ")
        text = replace_section_body(text, "## 是否允许 AI 继续", allow_ai)
        text = replace_section_body(text, "## 期望回退阶段", STAGE_BY_KEY[rollback_key].label)
        text = replace_section_body(text, "### 判定回退阶段", STAGE_BY_KEY[rollback_key].label)
        text = replace_section_body(text, "### 判定理由", f"根据驳回类型“{rejection_type}”和人工期望回退阶段判定。")
        text = replace_section_body(text, "### 下一步动作", must_fix or "根据驳回原因修改对应阶段产物。")
        text = replace_section_body(text, "### 是否需要人工澄清", "是" if allow_ai == "需要先提问澄清" else "否")
        if suggestion:
            text = replace_section_body(text, "## 可选建议", f"- {suggestion}")
        rejection_path.write_text(text, encoding="utf-8")
    except Exception as exc:
        print(f"workflow-reject: {exc}", file=sys.stderr)
        return 1

    print(f"rejected: {STAGE_BY_KEY[stage_key].label}")
    print(f"rollback_stage: {rollback_key}")
    return 0


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("task", help="task name")
    parser.add_argument("stage", help="stage key or label")
    parser.add_argument("--type", default="其他", help="rejection type")
    parser.add_argument("--reason", required=True, help="rejection reason")
    parser.add_argument("--must-fix", default="", help="required fix")
    parser.add_argument("--suggestion", default="", help="optional suggestion")
    parser.add_argument("--allow-ai", default="允许", choices=["允许", "不允许", "需要先提问澄清"])
    parser.add_argument("--rollback", default=None, help="expected rollback stage")
    parser.add_argument("--by", default="human", help="reviewer name")
    args = parser.parse_args()
    return reject(
        args.task,
        args.stage,
        args.type,
        args.reason,
        args.must_fix,
        args.suggestion,
        args.allow_ai,
        args.rollback,
        args.by,
    )


if __name__ == "__main__":
    raise SystemExit(main())
