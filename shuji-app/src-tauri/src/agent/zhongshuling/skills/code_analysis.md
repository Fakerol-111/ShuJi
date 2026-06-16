# Code Structure Analysis

Use this mode when asked to read and analyze target code, producing a structured analysis report. This is not a design document — it describes the current state, not what should be built.

## When to Use

- The task asks you to analyze existing code structure
- A workflow (optimization, refactor, bug fix) needs to establish baseline understanding before changes
- The Emperor or Cabinet asks "how does X work" or "analyze module Y"

## Work Method

1. Read the target files specified in the task
2. Browse directories to understand the file tree
3. For each file, extract:
   - Public exports (functions, classes, types)
   - Internal dependencies (what is imported from the project)
   - External dependencies (what is imported from outside)
   - Key data structures and their relationships
4. Create an analysis document via `create_document(type="anls")`
5. Fill in chunks via `append_document`

## Report Structure

The analysis document (`.shuji/analysis/`) must contain:
- **Scope**: Files analyzed, total lines of code
- **Module Map**: File tree with one-line descriptions
- **Public API Surface**: Each exported function/class/type with signature
- **Dependency Graph**: Which files depend on which files (internal only)
- **Data Flow**: Major data structures and how they flow through the code
- **Pain Points**: Any obvious issues (god files, circular dependencies, unclear boundaries)

## Quality Standards

- Every public export lists its exact signature
- Dependencies are traceable (file A imports from file B)
- The report describes "what is", not "what should be"
- No optimization suggestions, no refactoring proposals — only facts

## Routing

- Analysis complete -> Report document ID; routing depends on the caller's workflow

## Rules

- Do not propose changes or improvements — this is pure analysis
- Write the report in small chunks (500 characters per `append_document`)
- Read every file listed in the analysis — do not guess from filenames

## Output Block

At the end of each analysis, output the following structured summary:

```
Analysis Conclusion: <one-sentence core finding>
Number of Key Findings: <N>
Files Involved: <file name list>
Dependencies/Related Documents: <refs list>
```
