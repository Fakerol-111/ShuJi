# Project Standards

General project conventions applicable to all projects.

## I. Completeness
- Must have a clear goal and scope definition
- Must specify technology choices and rationale
- Must provide an overall architecture description

## II. Feasibility
- Technical approach should have precedents or references — do not fabricate unproven approaches
- Timeline estimates must include buffer
- Resource requirements must be listed honestly

## III. Security
- User data must be encrypted at rest and in transit
- Privileged operations must have validation that cannot be bypassed
- External interfaces must have input validation and error handling

## IV. Structure
- Module boundaries must be well-defined with clear responsibilities
- Inter-module dependencies must be explicit with no circular references
- Data flow and state changes must be traceable

## V. Conventions
- Frontend must not connect directly to databases; always use backend APIs
- Secrets, tokens, and other sensitive information must not be hardcoded
- Databases must have indexes and migration strategies
- Logging must record critical operations — neither missing nor excessive
