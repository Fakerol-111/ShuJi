# Optimize Workflow

Use this workflow for performance optimization, code profiling, or targeted non-architectural refactoring. The focus is analysis of existing code, not new feature design.

## Goal

Identify and resolve performance bottlenecks or code quality issues in existing implementation, without changing architecture or adding new capabilities.

## When to use

Use this mode when the emperor asks to:
- improve performance of a specific module or function
- profile and optimize resource usage
- clean up messy code without restructuring architecture
- reduce memory, CPU, or I/O overhead

Do NOT use this mode for architectural restructuring — use `workflow_refactor` instead.

## Workflow intent

Analysis-driven optimization: 中书令 profiles/reads existing code → produces optimization analysis → execution chain applies fixes.

## Steps

1. Create a task record with the optimization target and constraints
2. Record current baseline (e.g. "login endpoint takes ~800ms") if the emperor provided one
3. Route to `中书令` for performance analysis — NOT a full design, but an analysis of bottlenecks and recommended optimizations
4. When analysis returns, present findings to the emperor for approval. Use `<options>` if there are tradeoffs (e.g. speed vs readability)
5. After approval, route to `尚书令` for execution
6. When execution completes, summarize outcome

## 中书令 analysis expectation

The analysis should identify:
- specific bottlenecks with file/function locations
- root causes (algorithm, I/O, allocation, locking, etc.)
- recommended changes with expected impact
- any risks or tradeoffs

This is NOT a full architectural design — it is a focused optimization analysis.

## Routing policy

- Performance analysis → `route_to(to="中书令", subject="{id}")`
- Execution after approval → `route_to(to="尚书令", subject="{id}")`

## Escalation rule

If analysis reveals the problem requires architectural restructuring, escalate to `workflow_refactor`.

## Rules

- `route_to` and `<options>` are mutually exclusive in a single turn
- Do not skip imperial approval when tradeoffs exist
- Do not inflate a performance fix into a full redesign unless truly necessary
