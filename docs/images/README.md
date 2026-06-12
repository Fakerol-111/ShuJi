# 产品截图

将枢机界面截图或演示 GIF 放在此目录，供根目录 `README.md` 引用。

> `docs/` 其余内容在 `.gitignore` 中忽略（本地设计文档），**仅 `docs/images/` 会提交到仓库**。

## 建议文件

| 文件名 | 用途 | 建议尺寸 |
|--------|------|----------|
| `dashboard.png` | 主界面：聊天 + 部门状态 + 文档树 | 宽 1200px 左右 |
| `workflow.png` | 工作流进行中（部门协作、状态栏） | 同上 |
| `approval.png` | 朱批审批 / 决策面板（可选） | 同上 |
| `demo.gif` | 30–60 秒操作演示（可选） | 宽 ≤ 1200px |

## 在 README 中引用

截图就绪后，在 `README.md` 的「工作流概览」一节添加，例如：

```markdown
## 界面预览

![主界面](docs/images/dashboard.png)

![工作流演示](docs/images/demo.gif)
```

并删除「界面截图待补充」的占位说明。

## 拍摄建议

- 使用浅色或默认主题，保证文字清晰可读
- 可打码 API Key、项目路径等敏感信息
- PNG 用于静态图；GIF 或 MP4（GitHub 支持视频）用于演示
