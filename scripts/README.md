# 仓库脚本

与主应用构建无关的一次性 / 辅助脚本。主应用的构建与测试命令见 [`shuji-app/package.json`](../shuji-app/package.json) 与 [`CONTRIBUTING.md`](../CONTRIBUTING.md)。

| 脚本 | 说明 |
|------|------|
| [`add_speaker_notes.py`](add_speaker_notes.py) | 为验收答辩 `.pptx` 批量写入演讲者备注（需 `python-pptx`） |
| [`count_tests.sh`](count_tests.sh) | 统计 Rust + 前端测试数量，输出 JSON（文档引用，CI 调用） |
| [`../shuji-app/scripts/i18n-patch.mjs`](../shuji-app/scripts/i18n-patch.mjs) | 应用内 i18n 补丁（归属 `shuji-app`，非仓库根脚本） |

## 运行示例

```bash
pip install python-pptx
python scripts/add_speaker_notes.py
# 或指定 pptx 路径：
python scripts/add_speaker_notes.py path/to/deck.pptx
```
