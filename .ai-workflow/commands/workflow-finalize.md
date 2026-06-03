# workflow-finalize

Use this command to prepare final approval and submission artifacts.

Input:

```text
TASK=<task-name>
```

Procedure:

1. Run `make workflow-check TASK=<task-name>`.
2. Confirm current stage is `final_approval`.
3. Fill:
   - `07-final-approval.md`
   - `08-pr-description.md`
   - `commit-message.txt`
4. Run:

```bash
make workflow-submit-check TASK=<task-name>
```

Output:

- Report whether the task is ready for human final approval or submission.

Rules:

- Do not approve final approval yourself.
- Do not submit if `workflow-submit-check` fails.
