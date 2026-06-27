# delete-create 循环

## 现象

同一 path 上反复 `delete_file` → `create_file`，浪费 token 且易丢历史。

## 恢复步骤

1. **总结已知事实**：说明为何要改该文件、当前内容与目标差异
2. **换修改方式**：已有文件用 `edit_file` 或 `apply_patch`，勿 delete+create
3. **缩小范围**：一次 patch 只改必要片段
4. **仍循环**：停止删建，在报告中说明阻塞并请求协助
