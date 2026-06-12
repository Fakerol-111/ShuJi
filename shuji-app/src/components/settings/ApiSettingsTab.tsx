import { API_URL_PRESETS, MODEL_PRESETS } from '../../constants/presets';

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

interface ApiSettingsTabProps {
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
}

export default function ApiSettingsTab({
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
}: ApiSettingsTabProps) {
  const customCount = roleList.filter((r) => !(useDefault[r.key] ?? true)).length;

  return (
    <div className="space-y-3">
      {/* ── Default role ── */}
      <div className="space-y-1.5 pb-2 border-b border-ink-700">
        <span className="text-[11px] font-semibold text-ink-300">默认（全局）</span>
        <ConfigInput
          label="API 密钥"
          type="password"
          value={defaultCfg.api_key}
          onChange={(v) => setDefaultCfg({ ...defaultCfg, api_key: v })}
        />
        <ConfigInput
          label="API URL"
          value={defaultCfg.api_url}
          onChange={(v) => setDefaultCfg({ ...defaultCfg, api_url: v })}
        />
        <ModelSuggestions
          url={defaultCfg.api_url}
          model={defaultCfg.model}
          onSelect={(m) => setDefaultCfg({ ...defaultCfg, model: m })}
        />
        <div className="flex gap-1 flex-wrap mt-1">
          {API_URL_PRESETS.map((p) => (
            <button
              key={p.label}
              onClick={() =>
                setDefaultCfg({
                  ...defaultCfg,
                  api_url: p.url,
                  model: MODEL_PRESETS[p.url]?.[0] ?? defaultCfg.model,
                })
              }
              className={`text-[10px] px-2 py-0.5 rounded-full border transition-colors ${
                defaultCfg.api_url === p.url
                  ? 'bg-ink-700 text-ink-100 border-ink-600'
                  : 'bg-ink-800 text-ink-400 border-ink-700 hover:border-ink-500'
              }`}
            >
              {p.label}
            </button>
          ))}
        </div>
      </div>

      {/* ── Per-role overrides ── */}
      <div className="space-y-0.5">
        <div className="flex items-center justify-between mb-1">
          <span className="text-[11px] font-semibold text-ink-300">各角色覆盖</span>
          <span className="text-[10px] text-ink-500">
            {customCount}/{roleList.length} 个使用自定义
          </span>
        </div>

        {/* Batch actions */}
        {customCount > 0 && (
          <button
            onClick={onApplyDefaultToAll}
            className="w-full text-[10px] px-2 py-1 mb-1 rounded bg-ink-800 text-ink-400 hover:bg-ink-700 hover:text-ink-200 transition-colors"
          >
            全部恢复默认
          </button>
        )}

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
            <div key={r.key} className="border border-ink-800 rounded">
              <button
                onClick={() => setExpandedRole(isExpanded ? null : r.key)}
                className="w-full flex items-center gap-2 px-2 py-1.5 text-xs text-ink-300 hover:bg-ink-800 transition-colors"
              >
                <span className="text-ink-500 shrink-0">{isExpanded ? '▾' : '▸'}</span>
                <label
                  className="flex items-center gap-1.5 shrink-0"
                  onClick={(e) => e.stopPropagation()}
                >
                  <input
                    type="checkbox"
                    checked={usingDefault}
                    onChange={() => toggleDefault(r.key)}
                    className="accent-ink-500"
                  />
                  <span className="text-[10px] text-ink-500 whitespace-nowrap">使用默认</span>
                </label>
                <span className="flex-1 text-left">{r.label}</span>
                <span className="text-[10px] text-ink-500 italic">{provider}</span>
              </button>
              {isExpanded && (
                <div className="px-2 pb-2 space-y-1">
                  {usingDefault ? (
                    <div className="text-[10px] text-ink-500 italic px-1 py-2">
                      使用默认配置（{defaultCfg.model || '未设置'}）
                      <br />
                      取消勾选"使用默认"可单独设置
                    </div>
                  ) : (
                    <>
                      <ConfigInput
                        label="API 密钥"
                        type="password"
                        value={overrides[r.key]?.api_key ?? ''}
                        onChange={(v) => setOverride(r.key, 'api_key', v)}
                      />
                      <ConfigInput
                        label="API URL"
                        value={overrides[r.key]?.api_url ?? ''}
                        onChange={(v) => setOverride(r.key, 'api_url', v)}
                      />
                      <ModelSuggestions
                        url={overrides[r.key]?.api_url ?? ''}
                        model={overrides[r.key]?.model ?? ''}
                        onSelect={(m) => setOverride(r.key, 'model', m)}
                      />
                      <button
                        onClick={() => onApplyRoleToOthers(r.key)}
                        className="w-full text-[10px] px-2 py-1 mt-1 rounded bg-blue-900/30 text-blue-400 hover:bg-blue-900/50 transition-colors"
                      >
                        应用到所有其他角色
                      </button>
                    </>
                  )}
                </div>
              )}
            </div>
          );
        })}
      </div>
    </div>
  );
}

// ── Sub-components ─────────────────────────────────────

function ConfigInput({
  label,
  type,
  value,
  onChange,
}: {
  label: string;
  type?: 'text' | 'password';
  value: string;
  onChange: (value: string) => void;
}) {
  return (
    <label className="block">
      <span className="text-[10px] text-ink-500">{label}</span>
      <input
        type={type || 'text'}
        value={value}
        onChange={(e) => onChange(e.target.value)}
        className="w-full mt-0.5 px-2 py-1 text-xs bg-ink-800 border border-ink-700 rounded text-ink-200 focus:outline-none focus:border-ink-500"
      />
    </label>
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
      <label className="block">
        <span className="text-[10px] text-ink-500">模型</span>
        <div className="flex gap-1 flex-wrap mt-0.5">
          {suggestions.map((m) => (
            <button
              key={m}
              onClick={() => onSelect(m)}
              className={`text-[10px] px-2 py-0.5 rounded-full border transition-colors ${
                model === m
                  ? 'bg-ink-700 text-ink-100 border-ink-600'
                  : 'bg-ink-800 text-ink-400 border-ink-700 hover:border-ink-500'
              }`}
            >
              {m}
            </button>
          ))}
        </div>
      </label>
    );
  }
  return <ConfigInput label="模型" value={model} onChange={onSelect} />;
}
