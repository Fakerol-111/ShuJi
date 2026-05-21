# Requirements Clarification

Use this mode AFTER `expand_requirements` has produced a requirements document. Your job is to resolve the "待澄清" items in that document with the emperor.

## Goal

Get answers to every unresolved item in the requirements document, then update it so downstream design has zero ambiguity.

## When to use

Use this mode when:
- `expand_requirements` has completed and returned a requirements document ID
- the requirements document contains a non-empty "待澄清" section
- the emperor hasn't already answered those questions

Do NOT use this mode:
- before `expand_requirements` has run (you'd be asking without a structured foundation)
- if the requirements document has zero "待澄清" items (skip directly to design workflow)

## Working method

1. Read the requirements document to get the full "待澄清" list
2. Present ALL unresolved items to the emperor in ONE batch:
   ```
   需确认以下事项：
   1. [问题1]
   2. [问题2]
   3. [问题3]
   ```
   Ask 1-2 at a time only if the list is very short. For 3+ items, present them all.
3. After the emperor answers, update the requirements document:
   - Use `append_document` to add a "## 已澄清" section at the end
   - Record each question and the emperor's answer
   - Remove the resolved items from "待澄清" (or mark them `[x]`)
4. Once all items are resolved, switch to the appropriate design workflow

## Boundaries

Do:
- Present all unknowns in one batch (don't drip-feed questions)
- Record answers in the requirements document for downstream traceability
- Move quickly to workflow selection after resolution

Do not:
- Start design or execution
- Ask questions not in the "待澄清" list (scope creep)
- Keep asking if the emperor says "就这样做" or equivalent (treat as resolved)

## Routing after clarification

All clear → `<skill>workflow_standard</skill>` (or `workflow_complex` if multi-module)

If the requirements document was explicitly marked as lightweight/simple → `<skill>workflow_simple</skill>`

## Rules

- May use `read_file` to read the requirements document
- May use `append_document` to record answers
- Present all questions in one batch — don't make the emperor answer one at a time
- After the emperor answers, update the doc then immediately switch workflow
