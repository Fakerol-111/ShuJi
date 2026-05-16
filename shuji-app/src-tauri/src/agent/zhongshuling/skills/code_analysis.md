# Code Structure Analysis

Use this mode when asked to read and analyze target code, producing a structured analysis report. This is NOT a design document — it describes what exists, not what should be built.

## When to use

- The task asks you to analyze existing code structure
- A workflow (optimize, refactor, bugfix) needs a baseline understanding before changes
- The emperor or 内阁 asks "how does X work" or "analyze module Y"

## Working method

1. Read the target files specified in the task
2. Browse the directory to understand the file tree
3. For each file, extract:
   - Public exports (functions, classes, types)
   - Internal dependencies (what it imports from the project)
   - External dependencies (what it imports from outside)
   - Key data structures and their relationships
4. Create an analysis document via `create_document(type="anls")`
5. Populate it in chunks via `append_document`

## Report structure

The analysis document (`.shuji/analysis/`) must contain:
- **Scope**: files analyzed, total LOC
- **Module map**: file tree with one-line descriptions
- **Public API surface**: every exported function/class/type with signature
- **Dependency graph**: which files depend on which (internal only)
- **Data flow**: major data structures and how they move through the code
- **Pain points**: any obvious issues (god files, circular deps, unclear boundaries)

## Quality bar

- Every public export is listed with its exact signature
- Dependencies are traceable (file A imports from file B)
- The report describes what IS, not what SHOULD BE
- No optimization suggestions, no refactoring plans — just facts

## Routing

- Analysis complete → report the document ID back; routing depends on the calling workflow

## Rules

- Do not suggest changes or improvements — this is pure analysis
- Write the report in small chunks (500 chars per `append_document`)
- Read every file you list in the analysis — do not guess from file names
