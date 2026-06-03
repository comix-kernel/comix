# workflow-rejection

Use this command after a human rejects a workflow stage.

Input:

```text
TASK=<task-name>
STAGE=<stage>
REASON=<reason>
```

Procedure:

1. If the human has not already run `make workflow-reject`, ask them to run it or provide the required rejection fields.
2. Read `.ai-workflow/tasks/<task-name>/rejection.md`.
3. Run:

```bash
make workflow-next TASK=<task-name>
make workflow-check TASK=<task-name>
```

4. Work on the rollback stage only.

Rules:

- Do not override the human rollback decision.
- If rollback stage and rejection reason conflict, explain the conflict and ask for confirmation.
