import type { WorkflowConfig as WFConfig } from '../../types';

interface WorkflowSettingsTabProps {
  workflowIntent: string;
  setWorkflowIntent: (key: string) => void;
  workflowPreset: string;
  setWorkflowPresetLocal: (key: string) => void;
  modelPreset: string;
  setModelPresetLocal: (key: string) => void;
}

const INTENTS: { key: string; label: string; desc: string }[] = [
  { key: 'auto', label: '自动', desc: '根据任务描述自动推断意图。（默认）' },
  { key: 'greenfield_standard', label: '新功能', desc: '全新功能开发，走完整设计→审查→执行流程。' },
  {
    key: 'brownfield_optimize',
    label: '存量优化',
    desc: '对现有代码进行优化，跳过需求展开和门下审查。',
  },
  { key: 'bugfix', label: '缺陷修复', desc: '修复缺陷，直接路由到工部编码修复。' },
  { key: 'demo', label: '快速原型', desc: '快速原型/演示，最轻量流程。' },
];

const PRESETS: { key: WFConfig['governance']; label: string; desc: string }[] = [
  { key: 'full', label: '完整治理', desc: '所有流程必经审查。适合高复杂度任务。' },
  { key: 'standard', label: '标准', desc: '跳过门下审查。适合中等复杂度任务。（默认）' },
  { key: 'fast', label: '极速', desc: '跳过设计/审查，直达执行。适合小改动。' },
  { key: 'audit', label: '审计', desc: '强制审查和规范检查。适合合规场景。' },
];

const MODEL_PRESETS: { key: string; label: string; desc: string }[] = [
  { key: 'balanced', label: '均衡', desc: '全部部门使用同一模型（默认）' },
  { key: 'economy', label: '经济', desc: '审查/检查部门用轻量模型，设计/编码用默认' },
  { key: 'quality', label: '质量', desc: '设计/编码部门用最强模型，其余用默认' },
];

export default function WorkflowSettingsTab({
  workflowIntent,
  setWorkflowIntent,
  workflowPreset,
  setWorkflowPresetLocal,
  modelPreset,
  setModelPresetLocal,
}: WorkflowSettingsTabProps) {
  return (
    <div className="space-y-3">
      {/* ── Workflow Intent ── */}
      <div className="space-y-1">
        <span className="text-[11px] font-semibold text-ink-300">任务意图 (Intent)</span>
        <div className="flex gap-1 flex-wrap">
          {INTENTS.map((p) => (
            <button
              key={p.key}
              onClick={() => setWorkflowIntent(p.key)}
              className={`text-[10px] px-2 py-1 rounded-full border transition-colors ${
                workflowIntent === p.key
                  ? 'bg-ink-700 text-ink-100 border-ink-600'
                  : 'bg-ink-800 text-ink-400 border-ink-700 hover:border-ink-500'
              }`}
              title={p.desc}
            >
              {p.label}
            </button>
          ))}
        </div>
        <div className="text-[10px] text-ink-500 px-1">
          {INTENTS.find((i) => i.key === workflowIntent)?.desc || ''}
        </div>
      </div>

      {/* ── Workflow preset ── */}
      <div className="space-y-1 pt-2 border-t border-ink-700">
        <span className="text-[11px] font-semibold text-ink-300">流程预设</span>
        <div className="flex gap-1 flex-wrap">
          {PRESETS.map((p) => (
            <button
              key={p.key}
              onClick={() => setWorkflowPresetLocal(p.key)}
              className={`text-[10px] px-2 py-1 rounded-full border transition-colors ${
                workflowPreset === p.key
                  ? 'bg-ink-700 text-ink-100 border-ink-600'
                  : 'bg-ink-800 text-ink-400 border-ink-700 hover:border-ink-500'
              }`}
              title={p.desc}
            >
              {p.label}
            </button>
          ))}
        </div>
        <div className="text-[10px] text-ink-500 px-1">
          {PRESETS.find((p) => p.key === workflowPreset)?.desc || ''}
        </div>
      </div>

      {/* ── Model preset ── */}
      <div className="space-y-1 pt-2 border-t border-ink-700">
        <span className="text-[11px] font-semibold text-ink-300">模型分级预设</span>
        <div className="flex gap-1 flex-wrap items-center">
          {MODEL_PRESETS.map((p) => (
            <button
              key={p.key}
              onClick={() => setModelPresetLocal(p.key)}
              className={`text-[10px] px-2 py-1 rounded-full border transition-colors ${
                modelPreset === p.key
                  ? 'bg-ink-700 text-ink-100 border-ink-600'
                  : 'bg-ink-800 text-ink-400 border-ink-700 hover:border-ink-500'
              }`}
              title={p.desc}
            >
              {p.label}
            </button>
          ))}
          {modelPreset === 'custom' && (
            <span className="text-[10px] text-ink-400 italic">自定义</span>
          )}
        </div>
        <div className="text-[10px] text-ink-500 px-1">
          {MODEL_PRESETS.find((p) => p.key === modelPreset)?.desc || '已手动修改部门模型配置'}
        </div>
        <div className="text-[10px] text-ink-400 px-1">
          切换预设会覆盖相关角色的 model 字段，不改 API URL/Key。
        </div>
      </div>
    </div>
  );
}
