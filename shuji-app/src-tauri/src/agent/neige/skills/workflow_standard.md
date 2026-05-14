# Standard Workflow

Use this workflow when the task needs real governance: design first, then review, then execution. This is the default governed path for medium-complexity work.

## Goal

Ensure that meaningful business or structural work receives design before implementation, without forcing full multi-phase governance.

## When to use

Use this mode when most of the following are true:
- the task introduces meaningful business logic or multiple collaborating modules
- implementation should not start before design constraints are visible
- review is useful before execution
- the task is important enough to audit, but does not require explicit phase planning

## Workflow intent

This workflow adds design and review before execution, but keeps the path linear:
design -> review -> imperial approval if needed -> execution

## Steps

1. Create a task record
2. Record the emperor's goal, scope, constraints, and success criteria
3. Route to `中书令` for design
4. When design/review results return, read the review report. Then present the design and review to the emperor — imperial sign-off is required even if the review passed. Use `<options>` for the emperor to decide.
5. After approval or clear authorization, route to `尚书令`
6. When execution completes, summarize the outcome

## Design expectation

The design should establish enough structure that execution departments do not guess architecture or scope.

## Imperial decision points

Use `<options>` or a concise approval question when:
- a reviewed design is ready for approval
- a review raises alternatives or objections requiring imperial choice
- the next step materially changes scope, risk, or policy

If the emperor has already delegated approval policy clearly, continue accordingly.

## Routing policy

- Request design -> `route_to(to="中书令", subject="{id}")`
- Start execution after approval -> `route_to(to="尚书令", subject="{id}")`

## Escalation rule

If the returned design shows the task really needs phase planning or staged governance, escalate to `workflow_complex` instead of pretending standard workflow is enough.

## Rules

- `route_to` and `<options>` are mutually exclusive in a single turn
- Do not skip design approval when the design result obviously requires a decision
- Do not overcomplicate the task if the design remains bounded and linear
