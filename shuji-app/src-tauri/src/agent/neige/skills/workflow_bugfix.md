# Bugfix Workflow

Use this workflow for bug localization and repair. The flow adds diagnosis before execution and mandatory regression testing.

## Goal

Find the root cause, fix the bug, and ensure it stays fixed with a regression test.

## When to use

Use this mode when the emperor reports:
- a specific bug or unexpected behavior
- a crash, error, or incorrect output
- a regression from previous changes
- something that "used to work but doesn't anymore"

## Workflow intent

Route to 尚书令 for diagnosis and fix execution. 尚书令 orchestrates 兵部 (regression test contract first), then 工部 (fix), then 刑部 (regression test verification).

## Steps

1. Create a task record with bug description, reproduction steps, and observed vs expected behavior
2. Route to `尚书令` for diagnosis and fix execution. The task must instruct 尚书令 to prioritize: read the relevant code to diagnose root cause first, then 兵部 writes a regression test contract that reproduces the bug, then 工部 implements the fix, then 刑部 runs the regression test.
3. When execution completes, verify the regression test passed before summarizing to the emperor

## Routing policy

- Bugfix execution → `route_to(to="尚书令", subject="{id}")`

## Rules

- Never skip diagnosis and go straight to fix — root cause must be understood first
- A regression test is mandatory for every bugfix
- If diagnosis reveals the bug is actually a design flaw, escalate to appropriate workflow
- `route_to` and `<options>` are mutually exclusive in a single turn
