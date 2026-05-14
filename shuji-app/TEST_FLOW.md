# 枢机 标准测试流程

## 前置

```bash
cd shuji-app
# 确保 .env 已配置 API Key（已实现角色自动使用 DEFAULT 配置）
npm run tauri dev
```

---

## 测试：完整交付流程

### Step 0 — 准备工作区
1. 选择一个**全新的空目录**作为工作区（或删掉之前测试的 `.shuji/` 目录）
2. ✅ 顶部显示项目名，左侧"皇帝目标"为空

### Step 1 — 皇帝下达指令
3. 在**决策**页输入框输入：
   ```
   皇帝
我要做一个erp管理系统，有库存管理，记账管理功能，个人使用，本地电脑部署，用python实现就行，然后网页端互动。请先载入workflow_complex再开始工作
   ```
4. 按 Enter 发送
5. ✅ 内阁收到指令，可能反问澄清，也可能直接确认
6. ✅ 如果内阁反问（附选项），选对应的选项回答
7. ✅ 内阁确认后，左侧进度面板更新

### Step 2 — 整体方案设计
8. 系统自动推进：
   - **中书省**（tool-use，5 次 tool call 上限）设计方案
   - 用 `read_file` / `list_dir` 了解已有设计
   - 用 `write_file` 写入 `.shuji/designs/`
   - 完成后调用 `complete_design`
9. ✅ 终端能看到 `[api] tool_call` 日志，包含 token 计数
10. ✅ **门下省**审查方案，返回审查意见
11. ✅ 如果驳回，升级到皇帝决策
12. ✅ 消息下方出现选项：**A. 照准** **B. 照准，补充** **C. 不可行**

### Step 3 — 测试"补充"功能
13. 点击 **B. 照准，补充**
14. ✅ 弹出一个 textarea
15. 输入：`前端用 React + TypeScript，后端用 Go`
16. 点击 **确认提交**
17. ✅ 消息以 `B\n前端用 React...` 格式发送
18. ✅ 系统继续推进

### Step 4 — 阶段设计循环
19. 系统自动进入阶段 1 详细设计
20. ✅ 中书省再次调用 tool，读整体方案后产出阶段设计
21. ✅ 门下省审查 → 皇帝决策
22. 每次出现选项时选 **A. 照准**
23. ✅ 阶段执行自动推进

### Step 5 — 尚书省自治执行
24. 方案批准后，**尚书省**（tool-use dispatch）接管执行：
    ```
    尚书省 read_file 读阶段设计
    尚书省 dispatch("吏部", "拆解任务") → 产出任务清单
    尚书省 dispatch("兵部", "根据设计写测试用例") → 产出测试用例
    尚书省 dispatch("工部", "编码实现并通过测试") → 产出代码文件
    尚书省 dispatch("刑部", "异常处理检查") → 产出检查报告
    尚书省 dispatch("礼部", "规范检查") → 产出检查报告
    尚书省 dispatch("户部", "记录归档+资源统计") → 产出执行报告
    尚书省 complete_execution() → 执行完成
    ```
25. ✅ 终端能看到 `[shangshu] dispatch iter=X/10` 日志
26. ✅ 每个 dispatch 都有对应的部门 agent 执行日志
27. ✅ 工部使用 `write_file` 写入实际代码文件
28. ✅ 户部报告包含各部门的 Token 消耗统计
29. ✅ 全部完成后，自动进入下一阶段

### Step 6 — 测试讨论功能
30. 切换到 **讨论** Tab
31. 输入：
    ```
    你觉得这个项目的数据库用什么比较好？
    ```
32. ✅ 内阁回复讨论意见（不干扰决策页的工作流）
33. 切回 **决策** Tab，工作流状态不变

### Step 7 — 观察 Token 仪表盘
34. 调用 `get_token_stats` IPC（前端可展示仪表盘）
35. ✅ 数据格式：
    ```json
    {
      "zhongshu": { "prompt_tokens": 12345, "completion_tokens": 6789, "total_tokens": 19134, "call_count": 5 },
      "menxia": { "prompt_tokens": 2345, "completion_tokens": 567, "total_tokens": 2912, "call_count": 3 },
      "shangshu": { "prompt_tokens": 34567, "completion_tokens": 8901, "total_tokens": 43468, "call_count": 10 },
      "libup": { "prompt_tokens": 1234, "completion_tokens": 345, "total_tokens": 1579, "call_count": 1 },
      ...
    }
    ```

### Step 8 — 观察日志
36. 点顶部 **日志** 按钮
37. ✅ 左侧列出各角色日志文件
38. 点各日志文件查看详情

### Step 9 — 验证交付
39. 继续在决策页选 **A. 照准**，直到所有阶段完成
40. ✅ 出现"项目已全部完成，交付归档"
41. ✅ 左侧进度 100%

---

## 快速测试输入（一键复制）

```
帮我做一个待办事项管理系统
```

```
B
前端用 React + TypeScript，后端用 Go，数据库用 PostgreSQL
```

```
你觉得数据库用什么比较好？（在讨论 Tab 输入）
```

---

## 预期终端输出

```
[debug] loaded .env from .../.env (N vars)
[engine] process_and_advance: input='帮我做一个...' overall=NotStarted
[api] tool-call iter=1/5 msgs=3
[api] tool_call: list_dir args={"path":"."}
[api] tokens: prompt=1234 completion=56 total=1290
[api] tool_call: write_file args={"path":".shuji/designs/overall_design.md"...
[api] tool_call: complete_design args={"summary":"整体方案已完成..."}
[engine] calling Menxia agent...
[engine] Menxia agent returned (2477 chars)

...阶段批准后...

[engine] calling Shangshu for phase1 execution...
[shangshu] dispatch iter=1/10
[shangshu] tool_call: dispatch args={"ministry":"吏部",...
[shangshu] dispatch iter=2/10
[shangshu] tool_call: dispatch args={"ministry":"兵部",...
[shangshu] dispatch iter=3/10
[shangshu] tool_call: dispatch args={"ministry":"工部",...
[shangshu] dispatch iter=4/10
[shangshu] tool_call: dispatch args={"ministry":"刑部",...
[shangshu] dispatch iter=5/10
[shangshu] tool_call: dispatch args={"ministry":"礼部",...
[shangshu] dispatch iter=6/10
[shangshu] tool_call: dispatch args={"ministry":"户部",...
[shangshu] tool_call: complete_execution args={"summary":"阶段1执行完成"}
```

---

## 当前架构

```
设计阶段（硬编码流程）：
  皇帝 → 中书省(tool-use) → 门下省(审查) → 内阁(汇总) → 皇帝决策

执行阶段（尚书省自治）：
  皇帝 → 尚书省(tool-use dispatch，独立循环)
           ├─ 吏部(任务拆解)
           ├─ 兵部(先写测试用例)
           ├─ 工部(编码实现)
           ├─ 刑部(异常检查)
           ├─ 礼部(规范检查)
           └─ 户部(记录归档+资源统计)
         → 完成或上报问题

制司：独立权限机构，直接对皇帝负责，不在执行流水线中
```

## 当前限制

| 项 | 值 |
|---|---|
| tool call 上限（中书省） | 5 次 |
| tool call 上限（尚书省） | 10 次 |
| tool call 上限（工部） | 5 次 |
| 驳回升级 | 1 次即升级到皇帝 |
| 切换 Tab | 决策/讨论两页上下文隔离 |
| Token 仪表盘 | 后端已支持，前端待展示 |
| 制司 | 独立机构，尚未接入工作流 |
