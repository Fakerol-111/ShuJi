# Bug Diagnosis

Use this mode to independently diagnose a bug through systematic code reading. This is the 中书令-side diagnostic capability — distinct from 制司's independent investigation role in the bugfix workflow.

## When to use

- 内阁 routes a task asking you to diagnose a bug from a design/architecture perspective
- The bug may have architectural root causes rather than local implementation errors
- A broader system understanding is needed to trace the issue across modules

Note: For runtime bug diagnosis in the `workflow_bugfix` flow, 内阁 routes to 制司. 中书令 diagnosis is used when the bug likely involves design-level issues or cross-module architecture problems.

## Working method

1. Read the bug description and reproduction steps from the task
2. Identify candidate modules/files based on architectural knowledge
3. Read → Hypothesize → Verify:
   - Read relevant source files
   - Form a hypothesis about root cause
   - Read more code to verify or refute the hypothesis
   - Repeat until root cause is confirmed
4. Create a diagnosis document via `create_document(type="anls")`
5. Populate in chunks via `append_document`

## Diagnosis report structure

The report (`.shuji/analysis/`) must contain:
- **Bug summary**: observed vs expected behavior, one sentence
- **Root cause**: specific file, function, and logic flaw
- **Trace**: call chain from trigger to failure point
- **Contributing factors**: design issues that enabled the bug (if any)
- **Affected scope**: what else might be impacted
- **Fix guidance**: where to fix (file/function) and what constraint to preserve

## Diagnosis discipline

- Do NOT write fix code — describe where and what constraint matters
- Verify your hypothesis with actual code reads, not assumptions
- If the bug spans multiple modules, trace the full cross-module path
- If you cannot confirm root cause, say so explicitly — do not guess

## Routing

- Diagnosis complete → route back to the caller (内阁 or 尚书令)

## Rules

- Read every file in the trace path — do not guess from architecture knowledge alone
- Report root cause, not symptoms
- If the fix requires architectural change, flag it explicitly
