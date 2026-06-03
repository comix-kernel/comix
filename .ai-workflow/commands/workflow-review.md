# workflow-review

Use this command to perform AI code review for a task.

Input:

```text
TASK=<task-name>
```

Procedure:

1. Run `make workflow-check TASK=<task-name>`.
2. Confirm current stage is `ai_review`.
3. Review the task documents and code diff.
4. Fill `.ai-workflow/tasks/<task-name>/06-ai-review.md`.

Review priorities:

- Behavioral regressions
- ABI compatibility
- User pointer and buffer validation
- panic risk
- Locking and concurrency risk
- Resource leaks
- Missing tests
- Scope creep

Output:

- Findings first, ordered by severity.
- If no blocking issues exist, state that explicitly and mention residual risk.

Rules:

- Do not approve review yourself.
- Do not hide test gaps.
