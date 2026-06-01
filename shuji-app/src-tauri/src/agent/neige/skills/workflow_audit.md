# Audit Workflow

Use this workflow for code audit, security review, or compliance inspection. Unlike standard execution workflows, audit is inspection-driven: 礼部 leads the review with compliance check and behavioral analysis.

## Goal

Produce a thorough audit report covering code quality, security concerns, standards compliance, and behavioral correctness. This workflow does NOT modify code — it only inspects and reports.

## When to use

Use this mode when the emperor asks to:
- audit a specific module or the entire codebase
- perform a security review
- check compliance with standards or precepts
- review code written by external contributors
- inspect for potential vulnerabilities

## Workflow intent

Inspection-first, with optional fix follow-up: 礼部 performs the audit → findings compiled into report → emperor decides on fixes.

## Steps

1. Create a task record with audit scope, focus areas, and any specific concerns
2. Route to `礼部` for standards check + test coverage audit + behavioral review + security-sensitive pattern inspection. 礼部 reads all relevant precepts, contracts, designs, and source files, then produces an audit report.
3. When the report returns, compile findings into a summary for the emperor. Use `<options>` to let the emperor decide: approve as-is, or route fixes to `尚书令`.
4. If the emperor orders fixes, route to `尚书令` for execution
5. After fixes, optionally re-audit the changed files

## 礼部 audit scope

礼部's audit covers:
- Standards check (precept compliance)
- Test coverage audit
- Behavioral consistency review (implementation vs design)
- Security-sensitive patterns (hardcoded credentials, unsafe input handling, etc.)
- Permission and access control review

## Routing policy

- Standards audit → `route_to(to="礼部", subject="{id}")`
- Fix execution (if ordered) → `route_to(to="尚书令", subject="{id}")`

Note: For audit, 内阁 routes directly to 礼部 — this is inspection, not execution dispatch. 尚书令 is only involved if the emperor decides to fix issues.

## Rules

- `route_to` and `<options>` are mutually exclusive in a single turn
- Do not auto-route fixes without imperial decision
- Audit reports must be presented to the emperor before any action is taken
- If no issues are found, report the clean audit and close
