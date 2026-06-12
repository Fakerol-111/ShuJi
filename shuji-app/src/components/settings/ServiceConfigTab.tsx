import { API_URL_PRESETS, MODEL_PRESETS } from '../../constants/presets';
import type { WorkflowConfig as WFConfig } from '../../types';
import {
  SettingsSection,
  SettingsField,
  SettingsChip,
  SettingsHint,
  SettingsMuted,
  SettingsAccordion,
  SettingsCheckbox,
  SettingsAction,
} from './SettingsPrimitives';

interface RoleFormState {
  api_key: string;
  api_url: string;
  model: string;
}

interface RoleInfo {
  key: string;
  label: string;
  description: string;
}

interface ServiceConfigTabProps {
  defaultCfg: RoleFormState;
  setDefaultCfg: (cfg: RoleFormState) => void;
  overrides: Record<string, RoleFormState>;
  setOverride: (role: string, field: keyof RoleFormState, value: string) => void;
  useDefault: Record<string, boolean>;
  toggleDefault: (role: string) => void;
  expandedRole: string | null;
  setExpandedRole: (key: string | null) => void;
  roleList: RoleInfo[];
  onApplyDefaultToAll: () => void;
  onApplyRoleToOthers: (role: string) => void;
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

const MODEL_PRESET_OPTIONS: { key: string; label: string; desc: string }[] = [
  { key: 'balanced', label: '均衡', desc: '全部部门使用同一模型（默认）' },
  { key: 'economy', label: '经济', desc: '审查/检查部门用轻量模型，设计/编码用默认' },
  { key: 'quality', label: '质量', desc: '设计/编码部门用最强模型，其余用默认' },
];

export default function ServiceConfigTab({
  defaultCfg,
  setDefaultCfg,
  overrides,
  setOverride,
  useDefault,
  toggleDefault,
  expandedRole,
  setExpandedRole,
  roleList,
  onApplyDefaultToAll,
  onApplyRoleToOthers,
  workflowIntent,
  setWorkflowIntent,
  workflowPreset,
  setWorkflowPresetLocal,
  modelPreset,
  setModelPresetLocal,
}: ServiceConfigTabProps) {
  const customCount = roleList.filter((r) => !(useDefault[r.key] ?? true)).length;

  return (
    <div className="space-y-6">
      <SettingsSection title="全局默认">
        <div className="space-y-3">
          <SettingsField
            label="API 密钥"
            type="password"
            value={defaultCfg.api_key}
            onChange={(e) => setDefaultCfg({ ...defaultCfg, api_key: e.target.value })}
          />
          <SettingsField
            label="API URL"
            value={defaultCfg.api_url}
            onChange={(e) => setDefaultCfg({ ...defaultCfg, api_url: e.target.value })}
          />
          <ModelSuggestions
            url={defaultCfg.api_url}
            model={defaultCfg.model}
            onSelect={(m) => setDefaultCfg({ ...defaultCfg, model: m })}
          />
          <div className="flex gap-1.5 flex-wrap">
            {API_URL_PRESETS.map((p) => (
              <SettingsChip
                key={p.label}
                size="sm"
                selected={defaultCfg.api_url === p.url}
                onClick={() =>
                  setDefaultCfg({
                    ...defaultCfg,
                    api_url: p.url,
                    model: MODEL_PRESETS[p.url]?.[0] ?? defaultCfg.model,
                  })
                }
              >
                {p.label}
              </SettingsChip>
            ))}
          </div>
        </div>
      </SettingsSection>

      <SettingsSection title="模型分级预设" divider>
        <div className="flex gap-2 flex-wrap items-center">
          {MODEL_PRESET_OPTIONS.map((p) => (
            <SettingsChip
              key={p.key}
              selected={modelPreset === p.key}
              onClick={() => setModelPresetLocal(p.key)}
              title={p.desc}
            >
              {p.label}
            </SettingsChip>
          ))}
          {modelPreset === 'custom' && <span className="text-xs text-ink-600 italic">自定义</span>}
        </div>
        <SettingsHint>
          {MODEL_PRESET_OPTIONS.find((p) => p.key === modelPreset)?.desc ||
            '已手动修改部门模型配置'}
        </SettingsHint>
        <SettingsHint>切换预设会覆盖相关角色的 model 字段，不改 API URL/Key。</SettingsHint>
      </SettingsSection>

      <SettingsSection
        title="逐角色覆盖"
        description={`${customCount}/${roleList.length} 个角色使用自定义配置`}
        divider
      >
        {customCount > 0 && (
          <SettingsAction onClick={onApplyDefaultToAll} className="w-full text-center">
            全部恢复默认
          </SettingsAction>
        )}

        <div className="space-y-2">
          {roleList.map((r) => {
            const isExpanded = expandedRole === r.key;
            const usingDefault = useDefault[r.key] ?? true;
            const effective = !usingDefault && overrides[r.key] ? overrides[r.key] : defaultCfg;
            const provider = effective.api_url.includes('anthropic.com')
              ? 'Anthropic'
              : effective.api_url.includes('deepseek.com')
                ? 'DeepSeek'
                : effective.api_url.includes('openai.com')
                  ? 'OpenAI'
                  : '自定义';
            return (
              <SettingsAccordion
                key={r.key}
                expanded={isExpanded}
                onToggle={() => setExpandedRole(isExpanded ? null : r.key)}
                title={r.label}
                meta={provider}
                leading={
                  <SettingsCheckbox
                    checked={usingDefault}
                    onChange={() => toggleDefault(r.key)}
                    label="使用默认"
                    onClick={(e) => e.stopPropagation()}
                  />
                }
              >
                {usingDefault ? (
                  <SettingsMuted>
                    使用默认配置（{defaultCfg.model || '未设置'}）
                    <br />
                    取消勾选「使用默认」可单独设置
                  </SettingsMuted>
                ) : (
                  <>
                    <SettingsField
                      label="API 密钥"
                      type="password"
                      value={overrides[r.key]?.api_key ?? ''}
                      onChange={(e) => setOverride(r.key, 'api_key', e.target.value)}
                    />
                    <SettingsField
                      label="API URL"
                      value={overrides[r.key]?.api_url ?? ''}
                      onChange={(e) => setOverride(r.key, 'api_url', e.target.value)}
                    />
                    <ModelSuggestions
                      url={overrides[r.key]?.api_url ?? ''}
                      model={overrides[r.key]?.model ?? ''}
                      onSelect={(m) => setOverride(r.key, 'model', m)}
                    />
                    <SettingsAction
                      variant="accent"
                      onClick={() => onApplyRoleToOthers(r.key)}
                      className="w-full text-center"
                    >
                      应用到所有其他角色
                    </SettingsAction>
                  </>
                )}
              </SettingsAccordion>
            );
          })}
        </div>
      </SettingsSection>

      <SettingsSection title="任务意图" divider>
        <div className="flex gap-2 flex-wrap">
          {INTENTS.map((p) => (
            <SettingsChip
              key={p.key}
              selected={workflowIntent === p.key}
              onClick={() => setWorkflowIntent(p.key)}
              title={p.desc}
            >
              {p.label}
            </SettingsChip>
          ))}
        </div>
        <SettingsHint>{INTENTS.find((i) => i.key === workflowIntent)?.desc || ''}</SettingsHint>
      </SettingsSection>

      <SettingsSection title="流程治理" divider>
        <div className="flex gap-2 flex-wrap">
          {PRESETS.map((p) => (
            <SettingsChip
              key={p.key}
              selected={workflowPreset === p.key}
              onClick={() => setWorkflowPresetLocal(p.key)}
              title={p.desc}
            >
              {p.label}
            </SettingsChip>
          ))}
        </div>
        <SettingsHint>{PRESETS.find((p) => p.key === workflowPreset)?.desc || ''}</SettingsHint>
      </SettingsSection>
    </div>
  );
}

function ModelSuggestions({
  url,
  model,
  onSelect,
}: {
  url: string;
  model: string;
  onSelect: (model: string) => void;
}) {
  const suggestions = MODEL_PRESETS[url];
  if (suggestions && suggestions.length > 0) {
    return (
      <div className="space-y-1.5">
        <span className="text-xs font-medium text-ink-700">模型</span>
        <div className="flex gap-1.5 flex-wrap">
          {suggestions.map((m) => (
            <SettingsChip key={m} size="sm" selected={model === m} onClick={() => onSelect(m)}>
              {m}
            </SettingsChip>
          ))}
        </div>
      </div>
    );
  }
  return <SettingsField label="模型" value={model} onChange={(e) => onSelect(e.target.value)} />;
}
