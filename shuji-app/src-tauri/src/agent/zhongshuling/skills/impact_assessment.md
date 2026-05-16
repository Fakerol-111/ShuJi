# Impact Assessment

Use this mode to evaluate the impact scope of a proposed change — what files, modules, and behaviors would be affected. This is a risk-analysis tool, not a design tool.

## When to use

- Before a major refactoring, to understand what will be touched
- When the emperor or 内阁 asks "what would be affected if we change X"
- After a diagnosis identifies a fix, to assess the fix's blast radius
- Before merging a cross-module change

## Prerequisites

- A proposed change: could be a design document, a bug diagnosis, an optimization plan, or a direct request
- Understanding of the current code structure (read analysis documents or the source)

## Working method

1. Read the proposed change (design, diagnosis, optimization plan, or task description)
2. Identify the change points — what files/functions will be modified
3. For each change point, trace:
   - Callers: what depends on this function/signature
   - Callees: what this function depends on
   - Data impact: what data structures change, and what reads/writes them
   - API impact: any public API or contract changes
   - Test impact: which tests will need updating
4. Create an impact report via `create_document(type="anls")`
5. Populate in chunks via `append_document`

## Report structure

The impact report (`.shuji/analysis/`) must contain:
- **Change summary**: what is being proposed, one paragraph
- **Direct changes**: files/functions that will be modified
- **Ripple effects**: files that depend on the changed code (callers, importers)
- **API/contract impact**: any public signature changes, with migration notes
- **Test impact**: existing tests that will break or need updating
- **Risk assessment**: low/medium/high, with specific risk factors
- **Mitigation**: recommended order of changes to minimize risk

## Dependency tracing

- Use the project's import/dependency structure
- Start from the change point and walk outward
- Mark uncertain impacts as "needs verification" rather than guessing

## Routing

- Assessment complete → report back; routing depends on the calling workflow

## Rules

- Trace actual dependencies from code, not assumptions
- Mark uncertainty explicitly — do not present guesses as facts
- If the blast radius is unexpectedly large, recommend a smaller first step
