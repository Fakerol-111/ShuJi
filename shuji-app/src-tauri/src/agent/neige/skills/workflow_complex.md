# Complex Workflow

Use this workflow for high-impact, high-uncertainty, or multi-stage work that benefits from layered governance: overall design, phase planning when needed, then staged execution.

## Goal

Apply stronger governance to complex tasks while keeping the emperor in control at key decision points.

## When to use

Use this mode when one or more of the following are true:
- the task spans many modules or subsystems
- architecture may change materially
- staged delivery is necessary
- risk or uncertainty is high
- different phases need explicit review or approval
- downstream execution would be unsafe without architecture and phase decomposition first

## Workflow intent

This workflow is for tasks where one linear design step is insufficient.
Typical pattern:
overall design -> review -> phase planning/design as needed -> approval checkpoints -> execution

## Working method

1. Create a task record
2. Record goals, major modules, constraints, risks, and expected outcomes
3. Route to `中书令` for overall design
4. When design/review returns, present it to the emperor if approval is required
5. If approved, continue to phase planning or next governed design step
6. Repeat review/approval at major transitions when required
7. Route to `尚书令` only when the task is structured enough to execute safely
8. Summarize progress and outcome back to the emperor

## Emperor decision points

The emperor should usually be involved when:
- overall design is reviewed and ready for approval
- a phase plan materially defines delivery order and scope
- major objections, trade-offs, or scope changes appear

Do not auto-advance through every gate unless the emperor has clearly authorized that behavior.

## Routing policy

- Start design -> `route_to(to="中书令", subject="{id}")`
- Continue governed design -> `route_to(to="中书令", subject="{id}")`
- Start execution only after structure is stable -> `route_to(to="尚书令", subject="{id}")`

## Boundaries

Do:
- preserve governance strength
- expose major decision points
- use stronger process when risk justifies it

Do not:
- collapse a complex task into a simple execution shortcut
- over-specify phase details before design exists
- hide approval-worthy transitions from the emperor

## Rules

- `route_to` and `<options>` are mutually exclusive in a single turn
- If complexity turns out lower than expected, you may de-escalate to a lighter workflow
- If new risks appear, stay in governed mode rather than shortcutting execution
