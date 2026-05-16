# Refactor Workflow

Use this workflow for architectural restructuring of existing code. Unlike optimize (which targets performance) or standard (which builds new features), refactoring is about changing the shape of existing code.

## Goal

Restructure existing architecture while preserving or improving external behavior. The key difference from standard workflow: 中书令 first analyzes the current architecture, then designs the target architecture.

## When to use

Use this mode when the emperor asks to:
- restructure module boundaries
- change data flow or dependency direction
- split a monolith into modules, or merge over-split modules
- introduce a new abstraction layer across the codebase
- change the storage, messaging, or communication pattern

Do NOT use this for performance fixes (use `workflow_optimize`) or bug fixes (use `workflow_bugfix`).

## Workflow intent

Two-phase design: current-state analysis → target-state design → review → execution.

## Steps

1. Create a task record with refactoring goals, scope boundaries, and constraints (e.g. "preserve all existing API behavior", "no data migration")
2. Route to `中书令` for current-state analysis — 中书令 reads the existing code and documents the current architecture, coupling points, and pain points
3. When analysis returns, present to the emperor for confirmation that the analysis is accurate
4. Route to `中书令` for refactoring design — based on the confirmed analysis, produce the target architecture
5. When design returns, 中书令 routes to 门下侍中 for review
6. When review returns, present the design and review to the emperor for sign-off. Use `<options>`.
7. After approval, route to `尚书令` for execution
8. When execution completes, summarize outcome

## 中书令 analysis expectation

The current-state analysis should cover:
- module dependency graph
- data flow paths
- coupling hot-spots
- existing abstraction layers
- pain points that the refactoring should address

This is analysis, not design — it describes what IS, not what SHOULD BE.

## Routing policy

- Current-state analysis → `route_to(to="中书令", subject="{id}")`
- Refactoring design → `route_to(to="中书令", subject="{id}")`
- Execution after approval → `route_to(to="尚书令", subject="{id}")`

## Imperial decision points

- After current-state analysis: does the emperor confirm the analysis?
- After design + review: does the emperor approve the refactoring plan?

## Rules

- `route_to` and `<options>` are mutually exclusive in a single turn
- Current-state analysis MUST precede refactoring design — do not design against an unverified understanding
- If the refactoring scope expands to include new features, escalate to `workflow_complex`
- Do not skip review for architectural refactoring
