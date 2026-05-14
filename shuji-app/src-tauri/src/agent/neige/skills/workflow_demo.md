# Demo Workflow

Use this workflow for tiny, low-risk implementation tasks where heavy governance would cost more than the task itself.

## Goal

Route the emperor's request into the lightest execution path while preserving a minimal task record.

## When to use

Use this mode when most of the following are true:
- the task is very small and clearly scoped
- usually one file or a very limited artifact
- no external dependency or architectural decision matters much
- no database, auth, migration, or risky persistent change is involved
- failure is easy to inspect and fix

## Workflow intent

This is a controlled fast path, not a free-form shortcut. Keep a lightweight task record, then route to `尚书令` for dispatch.

## Steps

1. Create a task document
2. Record the emperor's request and scope
3. Route to `尚书令` for execution dispatch
4. When a result returns, summarize it to the emperor

## Task record guidance

Capture only what execution needs:
- requested output
- explicit scope limits
- important constraints
- what is intentionally not included

## Routing policy

- Implementation start -> `route_to(to="尚书令", subject="{id}")`
- Do not introduce design review unless new information shows the task was misclassified

## Reclassification rule

If new information reveals hidden complexity or risk, do not stubbornly stay in demo mode.
Switch to a stronger workflow instead.

## Rules

- No `<options>` unless the emperor must truly decide something
- Do not inflate the task into design-heavy workflow without cause
- When work completes, present a concise result and return to normal cabinet interaction
