# Pipeline 死锁排查

## 现象

所有剩余步骤均被阻塞，没有可执行的步骤。

## 原因

- 步骤之间存在循环依赖（A→B→A）
- 某步骤 failed 且 on_failure=abort
- 某步骤要求的前置条件永远无法满足

## 操作

1. 检查 `runtime.json` 中的 `step_status` 和 `error_log`
2. 识别导致死锁的依赖链
3. 调用 `update_pipeline_plan` 跳过死锁步骤或修改依赖
4. 或重新提交 plan
