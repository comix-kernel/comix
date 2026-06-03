"""Shared helpers for the repository-local AI workflow."""

from __future__ import annotations

import json
import re
from dataclasses import dataclass
from datetime import datetime, timezone
from pathlib import Path


SCRIPT_PATH = Path(__file__).resolve()
if SCRIPT_PATH.parent.parent.name == ".ai-workflow":
    WORKFLOW_ROOT = SCRIPT_PATH.parents[1]
    ROOT = WORKFLOW_ROOT.parent
else:
    ROOT = SCRIPT_PATH.parents[1]
    WORKFLOW_ROOT = ROOT / ".ai-workflow"
TASK_DIR = WORKFLOW_ROOT / "tasks"
TEMPLATE_DIR = WORKFLOW_ROOT / "templates"


@dataclass(frozen=True)
class Stage:
    key: str
    file: str
    label: str


STAGES = [
    Stage("requirement", "01-requirement.md", "需求文档"),
    Stage("spec", "02-spec.md", "规格文档"),
    Stage("implementation_plan", "03-implementation-plan.md", "实现计划"),
    Stage("impl_loop", "04-implementation-result.md", "实现结果"),
    Stage("qa", "05-qa-report.md", "QA"),
    Stage("ai_review", "06-ai-review.md", "AI Review"),
    Stage("final_approval", "07-final-approval.md", "最终批准"),
]

STAGE_BY_KEY = {stage.key: stage for stage in STAGES}
STAGE_ALIASES = {
    "requirement": "requirement",
    "需求": "requirement",
    "需求文档": "requirement",
    "spec": "spec",
    "规格": "spec",
    "规格文档": "spec",
    "implementation-plan": "implementation_plan",
    "implementation_plan": "implementation_plan",
    "plan": "implementation_plan",
    "实现计划": "implementation_plan",
    "impl": "impl_loop",
    "impl-loop": "impl_loop",
    "impl_loop": "impl_loop",
    "实现": "impl_loop",
    "实现结果": "impl_loop",
    "qa": "qa",
    "测试": "qa",
    "ai-review": "ai_review",
    "ai_review": "ai_review",
    "review": "ai_review",
    "ai review": "ai_review",
    "final": "final_approval",
    "final-approval": "final_approval",
    "final_approval": "final_approval",
    "最终批准": "final_approval",
}

APPROVED = {"批准", "通过", "approved", "pass", "passed"}
REJECTED = {"驳回", "不通过", "rejected", "fail", "failed"}
PENDING = {"待定", "pending", "todo"}

ROLLBACK_BY_REJECTION_TYPE = {
    "需求理解错误": "requirement",
    "规格不完整": "spec",
    "方案不合理": "implementation_plan",
    "范围失控": "implementation_plan",
    "实现错误": "impl_loop",
    "测试失败": "qa",
    "代码风险": "implementation_plan",
}


def now() -> str:
    return datetime.now(timezone.utc).isoformat(timespec="seconds")


def slugify(value: str) -> str:
    return re.sub(r"[^\w.-]+", "-", value.strip(), flags=re.UNICODE).strip("-").lower()


def normalize_stage(stage: str) -> str:
    key = STAGE_ALIASES.get(stage.strip().lower(), stage.strip())
    if key not in STAGE_BY_KEY:
        raise ValueError(f"unknown stage: {stage}")
    return key


def task_path(task: str) -> Path:
    return TASK_DIR / slugify(task)


def require_task(task: str) -> Path:
    path = task_path(task)
    if not path.exists():
        raise FileNotFoundError(f"task not found: {path}")
    return path


def stage_path(path: Path, stage_key: str) -> Path:
    return path / STAGE_BY_KEY[stage_key].file


def read_status_json(path: Path) -> dict:
    status_path = path / "status.json"
    if not status_path.exists():
        return {
            "task": path.name,
            "created_at": "",
            "current_stage": "requirement",
            "stages": {stage.key: "pending" for stage in STAGES},
        }
    return json.loads(status_path.read_text(encoding="utf-8"))


def write_status_json(path: Path, data: dict) -> None:
    (path / "status.json").write_text(
        json.dumps(data, ensure_ascii=False, indent=2) + "\n",
        encoding="utf-8",
    )


def extract_status(text: str) -> str:
    match = re.search(r"状态[：:]\s*([^\n\r]+)", text)
    if not match:
        match = re.search(r"结论[：:]\s*([^\n\r]+)", text)
    if not match:
        return "missing"
    return match.group(1).strip()


def classify_status(status: str) -> str:
    normalized = status.strip().lower()
    if normalized in APPROVED:
        return "approved"
    if normalized in REJECTED:
        return "rejected"
    if normalized in PENDING:
        return "pending"
    if "不通过" in status or "驳回" in status:
        return "rejected"
    if "批准" in status or "通过" in status:
        return "approved"
    return "pending"


def markdown_stage_status(path: Path, stage_key: str) -> str:
    file_path = stage_path(path, stage_key)
    if not file_path.exists():
        return "missing"
    return classify_status(extract_status(file_path.read_text(encoding="utf-8")))


def update_field(text: str, field: str, value: str) -> str:
    pattern = rf"({re.escape(field)}[：:])\s*([^\n\r]*)"
    if re.search(pattern, text):
        return re.sub(pattern, lambda match: f"{match.group(1)} {value}", text, count=1)
    return text.rstrip() + f"\n\n{field}：{value}\n"


def replace_section_body(text: str, heading: str, value: str) -> str:
    pattern = rf"({re.escape(heading)}\n\n)(.*?)(?=\n## |\n### |\Z)"
    if re.search(pattern, text, flags=re.DOTALL):
        return re.sub(
            pattern,
            lambda match: f"{match.group(1)}{value.strip()}\n",
            text,
            count=1,
            flags=re.DOTALL,
        )
    return text.rstrip() + f"\n\n{heading}\n\n{value.strip()}\n"


def set_stage_approval(path: Path, stage_key: str, status: str, approver: str, note: str) -> None:
    file_path = stage_path(path, stage_key)
    text = file_path.read_text(encoding="utf-8")
    text = update_field(text, "状态", status)
    text = update_field(text, "审批人", approver)
    text = update_field(text, "审批时间", now())
    if note:
        text = update_field(text, "备注", note)
    file_path.write_text(text, encoding="utf-8")


def next_stage_key(stage_key: str) -> str | None:
    keys = [stage.key for stage in STAGES]
    index = keys.index(stage_key)
    if index + 1 >= len(keys):
        return None
    return keys[index + 1]


def first_incomplete_stage(path: Path) -> str | None:
    for stage in STAGES:
        if markdown_stage_status(path, stage.key) != "approved":
            return stage.key
    return None


def infer_rollback_stage(rejection_type: str, preferred: str | None) -> str:
    if preferred:
        preferred = preferred.strip()
        if preferred and preferred != "不确定":
            return normalize_stage(preferred)
    return ROLLBACK_BY_REJECTION_TYPE.get(rejection_type.strip(), "implementation_plan")


def sync_json_from_markdown(path: Path) -> dict:
    data = read_status_json(path)
    stages = data.setdefault("stages", {})
    for stage in STAGES:
        stages[stage.key] = markdown_stage_status(path, stage.key)
    data["current_stage"] = first_incomplete_stage(path) or "complete"
    write_status_json(path, data)
    return data
