# Optimization Plan

Use this mode to produce a step-by-step optimization plan based on a code analysis report. This bridges the gap between "here's what the code looks like" and "here's how to make it better."

## When to use

- You have a completed code analysis (anls document) and need to plan optimizations
- The task asks for a specific performance improvement plan
- A workflow_optimize task reaches you with an analysis to build on

## Prerequisites

- An analysis document (anls) or equivalent understanding of the target code
- Clear optimization goals (speed, memory, I/O, code quality)

## Working method

1. Read the analysis document referenced in the task
2. Identify optimization targets:
   - Hot paths (most frequently executed code)
   - Bottlenecks (algorithmic, I/O, allocation)
   - Low-hanging fruit (quick wins with minimal risk)
3. Prioritize: impact vs risk vs effort
4. Create an optimization plan via `create_document(type="plan")`
5. Populate in chunks via `append_document`

## Plan structure

The plan document (`.shuji/designs/`) must contain:
- **Baseline**: current state summary (from analysis)
- **Goals**: specific, measurable targets (e.g. "reduce endpoint latency from 800ms to 200ms")
- **Optimization steps**: ordered list, each with:
  - Target file/function
  - What changes
  - Expected impact
  - Risk level (low/medium/high)
- **Verification**: how to measure success for each step
- **Rollback plan**: how to revert if optimization causes issues

## Risk management

- Lowest-risk optimizations first
- Each step should be independently verifiable
- Flag any step that changes public API behavior
- If a step requires architectural change, note that `workflow_refactor` may be more appropriate

## Routing

- Plan complete → report back; routing depends on the calling workflow

## Rules

- Do not implement optimizations — this is planning only
- Each step must be specific enough for 工部 to execute without guessing
- Measurable targets only — "make it faster" is not a goal
