# Optimization Plan

Use this mode to produce a step-by-step optimization plan based on a code analysis report. It bridges the gap between "what the code looks like" and "how to make it better".

## When to Use

- You have a completed code analysis (anls document) and need to plan optimizations
- The task requires a specific performance improvement plan
- A `workflow_optimize` task arrives with an analysis to base work on

## Prerequisites

- An analysis document (anls) or equivalent understanding of the target code
- Clear optimization goals (speed, memory, I/O, code quality)

## Work Method

1. Read the analysis document referenced in the task
2. Identify optimization targets:
   - Hot paths (most frequently executed code)
   - Bottlenecks (algorithm, I/O, allocation)
   - Low-hanging fruit (quick wins with minimal risk)
3. Prioritize: impact vs risk vs effort
4. Create an optimization plan via `create_document(type="plan")`
5. Fill in chunks via `append_document`

## Plan Structure

The plan document (`.shuji/designs/`) must contain:
- **Baseline**: Current state summary (from analysis)
- **Goal**: Specific, measurable targets (e.g., "reduce endpoint latency from 800ms to 200ms")
- **Optimization steps**: Ordered list, each step containing:
  - Target file/function
  - What to change
  - Expected impact
  - Risk level (low/medium/high)
- **Validation**: How to measure the success of each step
- **Rollback plan**: How to revert if optimization causes issues

## Risk Management

- Lowest-risk optimizations first
- Each step should be independently verifiable
- Flag any step that changes public API behavior
- If a step requires architecture change, note that `workflow_refactor` may be more appropriate

## Routing

- Plan complete -> Report back; routing depends on the caller's workflow

## Rules

- Do not implement optimizations — this is pure planning
- Each step must be specific enough for the Ministry of Works to execute without guessing
- Use only measurable goals — "make it faster" is not a goal

## Output Block

At the end of each optimization plan, output the following structured summary:

```
Optimization Conclusion: <one-sentence optimization direction>
Number of Optimization Steps: <N>
Highest Risk Item: <risk description, or "None">
Dependencies/Related Documents: <refs list>
Next Route: <target department, document ID>
```
