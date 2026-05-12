# Simple Workflow

Use this workflow for small but real implementation tasks that need execution coordination, but not formal architecture design.

## Goal

Send a low-risk, straightforward task into the execution chain through `尚书令`, while avoiding unnecessary design overhead.

## When to use

Use this mode when most of the following are true:
- the task spans multiple files or a modest feature slice
- business logic is straightforward
- architectural impact is limited
- no major redesign is required
- risk is low to moderate and manageable through execution/testing

## Workflow intent

This workflow skips design-first governance but still preserves controlled execution through the dispatch chain.

## Steps

1. Create a task record
2. Write clear scope, constraints, and completion target
3. Route to `尚书令`
4. When execution results return, summarize them to the emperor

## Task record guidance

Include:
- requested capability
- important files/modules if known
- explicit constraints
- notable risks if any
- what is out of scope

## Routing policy

- Start execution -> `route_to(to="尚书令", subject="{id}")`
- If hidden architecture issues appear, escalate to a stronger workflow rather than forcing execution blindly

## Escalation triggers

Escalate out of simple workflow if you discover:
- core module boundary changes
- data model redesign
- multi-stage delivery need
- review/approval necessity before coding

## Rules

- No mandatory design review in this mode
- No `<options>` unless imperial choice is genuinely needed
- Prefer the lightest controlled path that is still safe
