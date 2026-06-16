# Impact Assessment

Assess the impact scope of a proposed change — which files, modules, and behaviors will be affected. This is a risk analysis tool, not a design tool.

## When to Use

- Before a major refactor, to understand what will be touched
- The Emperor or Cabinet asks "if we change X, what will be affected"
- After diagnosis determines a fix, to assess the blast radius of the fix
- Before merging cross-module changes

## Prerequisites

- A proposed change: can be a design document, defect diagnosis, optimization plan, or direct request
- Understanding of the current code structure (read analysis documents or source code)

## Work Method

1. Read the proposed change (design, diagnosis, optimization plan, or task description)
2. Identify change points — which files/functions will be modified
3. For each change point, trace:
   - Callers: What depends on this function/signature
   - Callees: What this function depends on
   - Data impact: Which data structures change, and what reads/writes them
   - API impact: Any public API or contract changes
   - Test impact: Which tests need updating
4. Create an impact report via `create_document(type="anls")`
5. Fill in chunks via `append_document`

## Report Structure

The impact report (`.shuji/analysis/`) must contain:
- **Change Summary**: What is proposed, one paragraph
- **Direct Impact**: Files/functions that will be modified
- **Ripple Effects**: Files that depend on the changed code (callers, importers)
- **API/Contract Impact**: Any public signature changes with migration notes
- **Test Impact**: Existing tests that will break or need updating
- **Risk Assessment**: Low/medium/high, with specific risk factors
- **Mitigation Measures**: Recommended change order to minimize risk

## Dependency Tracing

- Use the project's import/dependency structure
- Work outward from the change point
- Mark uncertain impacts as "needs verification" rather than guessing

## Routing

- Assessment complete -> Report back; routing depends on the caller's workflow

## Rules

- Trace actual dependencies from code, not assumptions
- Explicitly mark uncertainty — do not present guesses as facts
- If ripple effects are unexpectedly large, recommend a smaller first step

## Output Block

At the end of each impact assessment, output the following structured summary:

```
Assessment Conclusion: <one-sentence impact scope>
Directly Affected Files: <N>
Ripple Effect Files: <N>
API/Contract Changes: <Yes/No>
Risk Level: <Low/Medium/High>
Dependencies/Related Documents: <refs list>
```
