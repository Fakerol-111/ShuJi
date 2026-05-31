import { useState } from "react";

interface DeptInfo {
  name: string;
  aliases: string[];
  role: string;
}

const DEPARTMENTS: DeptInfo[] = [
  { name: "内阁", aliases: ["neige"], role: "总揽全局，决定工作流程和路由方向" },
  { name: "中书令", aliases: ["zhongshuling"], role: "方案设计，制定实现计划和架构" },
  { name: "门下侍中", aliases: ["menxiashizhong"], role: "设计审查，检查方案漏洞" },
  { name: "尚书令", aliases: ["shangshuling"], role: "执行调度，协调各部门工作" },
  { name: "吏部尚书", aliases: ["libushangshu"], role: "详细设计，细化技术方案" },
  { name: "兵部尚书", aliases: ["bingbushangshu"], role: "测试与契约，生成测试用例" },
  { name: "工部尚书", aliases: ["gongbushangshu"], role: "编码实现，写真正的代码" },
  { name: "刑部尚书", aliases: ["xingbushangshu"], role: "测试验证，运行测试确认通过" },
  { name: "礼部尚书", aliases: ["liburshangshu"], role: "规范检查，确保代码风格合规" },
  { name: "制司", aliases: ["zhisi"], role: "独立调查，特殊任务执行" },
];

const WORKFLOW_STEPS = [
  { step: "皇帝下旨", dept: "→ 内阁", desc: "分析需求，选择工作流程（标准/简单/bug修复）" },
  { step: "方案设计", dept: "→ 中书令", desc: "编写设计方案文档" },
  { step: "审查", dept: "→ 门下侍中", desc: "审查方案，给出意见" },
  { step: "皇帝审批", dept: "→ 你", desc: "查看方案文档，在预览区批准或驳回" },
  { step: "执行", dept: "→ 尚书令 → 各部", desc: "分工执行：详细设计→测试→编码→验证→规范" },
  { step: "完成", dept: "", desc: "结果汇报给皇帝" },
];

export default function HelpDrawer() {
  const [open, setOpen] = useState(false);

  return (
    <div className="relative">
      <button
        onClick={() => setOpen(!open)}
        className="text-xs px-2 py-1 text-ink-400 hover:text-ink-100 hover:bg-ink-800 rounded transition-colors"
        title="系统帮助"
      >
        ?
      </button>

      {open && (
        <>
          {/* Backdrop */}
          <div className="fixed inset-0 z-40" onClick={() => setOpen(false)} />
          {/* Drawer */}
          <div className="absolute right-0 top-8 z-50 w-80 bg-white border border-ink-200 rounded-xl shadow-xl shadow-ink-200/40 max-h-[80vh] overflow-y-auto">
            <div className="p-4 space-y-4">
              <div className="flex items-center justify-between">
                <h2 className="text-sm font-bold text-ink-900">三省六部系统帮助</h2>
                <button onClick={() => setOpen(false)} className="text-ink-400 hover:text-ink-600 text-sm">&times;</button>
              </div>

              {/* Workflow */}
              <Section title="工作流程">
                <div className="space-y-2">
                  {WORKFLOW_STEPS.map((ws, i) => (
                    <div key={i} className="flex gap-2 text-xs">
                      <span className="text-ink-400 w-3 shrink-0">{i + 1}.</span>
                      <div className="min-w-0">
                        <span className="font-medium text-ink-800">{ws.step}</span>
                        {ws.dept && <span className="text-vermillion ml-1">{ws.dept}</span>}
                        <div className="text-ink-500">{ws.desc}</div>
                      </div>
                    </div>
                  ))}
                </div>
              </Section>

              {/* Departments */}
              <Section title="部门对照表">
                <div className="space-y-1">
                  {DEPARTMENTS.map((d) => (
                    <div key={d.name} className="flex gap-2 text-xs py-1">
                      <span className="font-medium text-ink-800 w-14 shrink-0">{d.name}</span>
                      <span className="text-ink-500">{d.role}</span>
                    </div>
                  ))}
                </div>
              </Section>

              {/* Quick tips */}
              <Section title="快速提示">
                <ul className="text-xs text-ink-600 space-y-1 list-disc pl-4">
                  <li>输入 /level-1 到 /level-3 控制参与度</li>
                  <li>讨论 tab 仅聊天，不改代码</li>
                  <li>重要节点皇帝需审批文档</li>
                  <li>可随时点击「中断所有操作」停止处理</li>
                  <li>侧边栏可浏览所有 .shuji 文档</li>
                </ul>
              </Section>
            </div>
          </div>
        </>
      )}
    </div>
  );
}

function Section({ title, children }: { title: string; children: React.ReactNode }) {
  const [collapsed, setCollapsed] = useState(false);
  return (
    <div className="border border-ink-100 rounded-lg overflow-hidden">
      <button
        onClick={() => setCollapsed(!collapsed)}
        className="w-full flex items-center justify-between px-3 py-2 bg-ink-50 text-xs font-semibold text-ink-700 hover:bg-ink-100 transition-colors"
      >
        {title}
        <span className="text-ink-400">{collapsed ? "+" : "-"}</span>
      </button>
      {!collapsed && <div className="p-3">{children}</div>}
    </div>
  );
}
