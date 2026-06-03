# workflow-plan

Use this command to create or revise the implementation plan.

Input:

```text
TASK=<task-name>
```

Procedure:

1. Run `make workflow-check TASK=<task-name>`.
2. Confirm `requirement` and `spec` are approved and current stage is `implementation_plan`.
3. Read:
   - `01-requirement.md`
   - `02-spec.md`
4. Inspect the relevant code paths.
5. Edit `03-implementation-plan.md`.

The plan must include:

- Affected modules
- Files to modify
- Data structure changes
- Call-chain changes
- Concurrency and lock impact
- unsafe boundaries
- Test strategy
- Verification commands
- Risks
- Rollback method

Rules:

- Do not implement code in this stage.
- Keep scope aligned with approved requirement and spec.
