# workflow-requirement

Use this command to create or revise the requirement document for a task.

Input:

```text
TASK=<task-name>
REQUEST=<human natural language request>
```

Procedure:

1. Run `make workflow-next TASK=<task-name>`.
2. Confirm the current stage is `requirement`.
3. Edit `.ai-workflow/tasks/<task-name>/01-requirement.md`.
4. Capture:
   - Background
   - Goals
   - Non-goals
   - Use cases
   - Functional boundaries
   - Acceptance criteria
   - Risks and constraints

Output:

- Summarize the requirement document.
- Ask the human to approve or reject with:

```bash
make workflow-approve TASK=<task-name> STAGE=requirement BY=<name>
make workflow-reject TASK=<task-name> STAGE=requirement REASON="..."
```

Rules:

- Do not write code in this stage.
- Do not approve the stage yourself.
