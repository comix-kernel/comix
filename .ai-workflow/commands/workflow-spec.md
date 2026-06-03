# workflow-spec

Use this command to create or revise the specification document.

Input:

```text
TASK=<task-name>
```

Procedure:

1. Run `make workflow-check TASK=<task-name>`.
2. Confirm `requirement` is approved and current stage is `spec`.
3. Read `.ai-workflow/tasks/<task-name>/01-requirement.md`.
4. Edit `.ai-workflow/tasks/<task-name>/02-spec.md`.
5. Define:
   - Interface
   - Inputs
   - Outputs
   - Error codes
   - State changes
   - Edge cases
   - Linux ABI compatibility, when relevant
   - Test matrix

Output:

- Summarize important behavior and unresolved questions.
- Ask the human to approve or reject.

Rules:

- Do not start implementation.
- If behavior cannot be specified, ask for clarification instead.
