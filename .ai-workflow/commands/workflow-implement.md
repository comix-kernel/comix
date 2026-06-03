# workflow-implement

Use this command to perform the implementation loop.

Input:

```text
TASK=<task-name>
```

Procedure:

1. Run `make workflow-check TASK=<task-name>`.
2. Confirm `requirement`, `spec`, and `implementation_plan` are approved.
3. Read:
   - `01-requirement.md`
   - `02-spec.md`
   - `03-implementation-plan.md`
4. Modify code according to the approved plan.
5. Run focused validation when practical.
6. Record the result in `04-implementation-result.md`.

Implementation result must include:

- Code change summary
- Modified files
- Implementation notes
- Self-check result
- Known issues
- Whether the task is ready for QA

Rules:

- If the implementation plan is wrong, stop and request rejection or rollback to `implementation_plan`.
- Do not approve `impl_loop` yourself.
