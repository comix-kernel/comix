# workflow-start

Use this command when starting or resuming an AI workflow task.

Input:

```text
TASK=<task-name>
```

Procedure:

1. Run:

```bash
make workflow-next TASK=<task-name>
make workflow-check TASK=<task-name>
```

2. Read:

```text
.ai-workflow/AGENTS.md
.ai-workflow/tasks/<task-name>/status.json
.ai-workflow/tasks/<task-name>/<current-stage-file>
```

3. Report the current stage, blocking issues, and the next action.

Rules:

- Do not approve stages.
- Do not skip rejected or pending stages.
- Ask for the task name if it is missing.
