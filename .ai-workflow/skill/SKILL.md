---
name: ai-workflow
description: Use when working on repository tasks managed by .ai-workflow, especially Claude Code or Codex style AI coding workflows with staged requirement, specification, implementation plan, implementation loop, QA, AI review, human approval, rejection rollback, and submission checks.
metadata:
  short-description: Follow staged AI coding workflow
---

# AI Workflow Skill

Use this skill when the user asks to run, continue, review, or integrate a task under `.ai-workflow/`.

## Start

Require a task name. If missing, ask for it.

Run:

```bash
make workflow-next TASK=<task>
make workflow-check TASK=<task>
```

Read:

```text
.ai-workflow/AGENTS.md
.ai-workflow/tasks/<task>/status.json
.ai-workflow/tasks/<task>/<current-stage-file>
```

## Stage Commands

The command prompt files in `.ai-workflow/commands/` define the expected behavior for each stage:

- `workflow-start.md`: start or resume a task.
- `workflow-requirement.md`: write requirement document.
- `workflow-spec.md`: write specification document.
- `workflow-plan.md`: write implementation plan.
- `workflow-implement.md`: implement code and record result.
- `workflow-qa.md`: run QA and complete report.
- `workflow-review.md`: perform AI review.
- `workflow-finalize.md`: prepare final artifacts and submit check.
- `workflow-rejection.md`: handle rejected stages and rollback.

Read only the command file for the current action.

## Rules

- Never approve on behalf of the human. Do not run `make workflow-approve` unless the user explicitly asks you to execute their approval command.
- Do not implement code until `requirement`, `spec`, and `implementation_plan` are approved.
- If a stage is rejected, read `rejection.md` and work only on the rollback stage.
- During QA, run `make workflow-verify TASK=<task>` unless explicitly told not to.
- Before submission, run `make workflow-submit-check TASK=<task>`.

## Stage Map

```text
requirement -> 01-requirement.md
spec -> 02-spec.md
implementation_plan -> 03-implementation-plan.md
impl_loop -> 04-implementation-result.md
qa -> 05-qa-report.md
ai_review -> 06-ai-review.md
final_approval -> 07-final-approval.md, 08-pr-description.md, commit-message.txt
```
