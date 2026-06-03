# AI Workflow Agent Rules

These rules apply when an AI coding agent works on a task managed by `.ai-workflow/`.

## Hard Rules

- Start by running `make workflow-next TASK=<task>` and `make workflow-check TASK=<task>`.
- Work only on the current incomplete stage unless the user explicitly redirects the task.
- Do not run `make workflow-approve`; only a human reviewer may approve.
- Do not bypass rejected stages. Read `.ai-workflow/tasks/<task>/rejection.md` and follow the rollback stage.
- Do not implement code until `requirement`, `spec`, and `implementation_plan` are approved.
- During QA, run `make workflow-verify TASK=<task>` unless the user explicitly says not to; document any inability to run it.
- Before final submission, run `make workflow-submit-check TASK=<task>`.

## Stage Outputs

- `requirement`: edit `01-requirement.md`.
- `spec`: edit `02-spec.md`.
- `implementation_plan`: edit `03-implementation-plan.md`.
- `impl_loop`: modify code and record results in `04-implementation-result.md`.
- `qa`: run verification and complete `05-qa-report.md`.
- `ai_review`: review risks and findings in `06-ai-review.md`.
- `final_approval`: prepare `07-final-approval.md`, `08-pr-description.md`, and `commit-message.txt`.
