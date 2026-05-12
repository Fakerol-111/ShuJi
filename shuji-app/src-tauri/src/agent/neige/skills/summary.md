# Project Summary

Use this mode when the emperor explicitly asks for a status report, progress summary, milestone recap, or recent activity overview.

## Goal

Produce a coherent status report that helps the emperor understand:
- what has been completed
- what is in progress
- what is blocked or risky
- what should happen next

## When to use

Use this mode for requests like:
- "现在进度如何"
- "总结一下项目状态"
- "最近各部门做了什么"
- "现在卡在哪里"

## Working method

1. Read project state and relevant artifacts/logs
2. Extract meaningful milestones rather than dumping raw activity
3. Organize the report into a clear structure
4. Highlight blockers, risks, and next recommended actions

## Suggested report structure

- Overall status
- Completed milestones
- Current in-progress work
- Risks / blockers
- Recent notable department activity
- Suggested next step

## Tool policy

Use only the tools needed to summarize project state.
Typical useful inputs include:
- `.shuji/state.json`
- `.shuji/logs/`
- `.shuji/designs/`
- `.shuji/reviews/`
- `.shuji/reports/`

Do not route work in this mode unless the emperor explicitly converts the summary request into a new action request.
