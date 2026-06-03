# workflow-qa

Use this command to execute QA for a task.

Input:

```text
TASK=<task-name>
```

Procedure:

1. Run `make workflow-check TASK=<task-name>`.
2. Confirm current stage is `qa`.
3. Run:

```bash
make workflow-verify TASK=<task-name>
```

4. Complete `.ai-workflow/tasks/<task-name>/05-qa-report.md`.
5. Compare results against acceptance criteria and the spec test matrix.

Output:

- State whether QA appears passing or failing.
- If failing, recommend rollback target and required fixes.

Rules:

- Do not approve QA yourself.
- If verification cannot run, document the reason in the QA report.
