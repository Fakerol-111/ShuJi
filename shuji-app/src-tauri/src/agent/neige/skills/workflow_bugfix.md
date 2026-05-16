# Bugfix Workflow

Use this workflow for bug localization and repair. The flow adds independent diagnosis before execution and mandatory regression testing.

## Goal

Find the root cause, fix the bug, and ensure it stays fixed with a regression test.

## When to use

Use this mode when the emperor reports:
- a specific bug or unexpected behavior
- a crash, error, or incorrect output
- a regression from previous changes
- something that "used to work but doesn't anymore"

## Workflow intent

Diagnose first, then fix with regression guard: 制司 diagnoses independently → 尚书令 executes fix with regression test priority.

## Steps

1. Create a task record with bug description, reproduction steps, and observed vs expected behavior
2. Route to `制司` for independent diagnosis — 制司 reads the relevant code, traces the bug, and produces a diagnostic report
3. When diagnosis returns, read the report. If root cause is clear, present to the emperor and proceed. If diagnosis is inconclusive, present findings and ask for more information.
4. Route to `尚书令` for fix execution. The task must instruct 尚书令 to prioritize: 兵部 writes a regression test contract FIRST (a test that reproduces the bug and must pass after the fix), then 工部 implements the fix, then 刑部 runs the regression test.
5. When execution completes, verify the regression test passed before summarizing to the emperor

## 制司 diagnosis expectation

制司 should:
- read relevant source files to trace the bug
- identify root cause with file/function references
- suggest the likely fix location
- NOT write any code — only diagnose

## 尚书令 task guidance

The task routed to 尚书令 must include:
- the bug description and 制司's diagnostic report ref
- explicit instruction: 兵部 writes regression contract first, then 工部 fixes
- requirement: the regression test must fail before the fix and pass after

## Routing policy

- Bug diagnosis → `route_to(to="制司", subject="{id}")`
- Fix execution → `route_to(to="尚书令", subject="{id}")`

## Rules

- Never skip diagnosis and go straight to fix — root cause must be understood first
- A regression test is mandatory for every bugfix
- If diagnosis reveals the bug is actually a design flaw, escalate to appropriate workflow
- `route_to` and `<options>` are mutually exclusive in a single turn
