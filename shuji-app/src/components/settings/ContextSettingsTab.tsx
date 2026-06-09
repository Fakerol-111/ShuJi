import { ALL_ROLES, ROLE_CONTEXT_DEFAULTS } from '../../constants';

interface ContextRoleForm {
  token_threshold: number;
  keep_recent_count: number;
  mid_run_compact: boolean;
}

const DEFAULT_CONTEXT_VALUES: ContextRoleForm = {
  token_threshold: 750_000,
  keep_recent_count: 24,
  mid_run_compact: false,
};

interface ContextSettingsTabProps {
  contextOverrides: Record<string, ContextRoleForm>;
  contextUseDefault: Record<string, boolean>;
  toggleContextDefault: (roleKey: string) => void;
  setContextOverride: (role: string, field: string, value: number | boolean) => void;
  expandedRole: string | null;
  setExpandedRole: (key: string | null) => void;
  savedMsg: string;
  setSavedMsg: (msg: string) => void;
}

export default function ContextSettingsTab({
  contextOverrides,
  contextUseDefault,
  toggleContextDefault,
  setContextOverride,
  expandedRole,
  setExpandedRole,
  setSavedMsg,
}: ContextSettingsTabProps) {
  const roleList = ALL_ROLES.filter((r) => r.key !== 'default');

  const effectiveContext = (key: string): ContextRoleForm => {
    if (!(contextUseDefault[key] ?? true) && contextOverrides[key]) {
      return contextOverrides[key];
    }
    const role = ALL_ROLES.find((r) => r.key === key);
    if (role && ROLE_CONTEXT_DEFAULTS[role.label]) {
      const d = ROLE_CONTEXT_DEFAULTS[role.label];
      return {
        token_threshold: d.token_threshold,
        keep_recent_count: d.keep_recent_count,
        mid_run_compact: d.mid_run_compact,
      };
    }
    return DEFAULT_CONTEXT_VALUES;
  };

  return (
    <div className="space-y-0.5">
      <span className="text-[11px] font-semibold text-ink-300">上下文窗口配置</span>
      <div className="text-[10px] text-ink-500 px-1 pb-1">
        全局回退: {DEFAULT_CONTEXT_VALUES.token_threshold.toLocaleString()} tokens · cl100k ·
        DeepSeek 1M 接近上限再压缩
      </div>
      {roleList.map((r) => {
        const isExpanded = expandedRole === r.key;
        const usingDefault = contextUseDefault[r.key] ?? true;
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
                  onChange={() => toggleContextDefault(r.key)}
                  className="accent-ink-500"
                />
                <span className="text-[10px] text-ink-500 whitespace-nowrap">使用默认</span>
              </label>
              <span className="flex-1 text-left">{r.label}</span>
              <span className="text-[10px] text-ink-500 italic">
                {effectiveContext(r.key).token_threshold.toLocaleString()} tokens
              </span>
            </button>
            {isExpanded && (
              <div className="px-2 pb-2 space-y-1">
                {usingDefault ? (
                  <div className="text-[10px] text-ink-500 italic px-1 py-2">
                    使用部门内置推荐值（
                    {effectiveContext(r.key).token_threshold.toLocaleString()} tokens，保留{' '}
                    {effectiveContext(r.key).keep_recent_count} 条）
                    <br />
                    取消勾选"使用默认"可单独覆盖
                  </div>
                ) : (
                  <>
                    <ContextInput
                      label="压缩阈值（tokens）"
                      value={
                        contextOverrides[r.key]?.token_threshold ??
                        effectiveContext(r.key).token_threshold
                      }
                      onChange={(v) => setContextOverride(r.key, 'token_threshold', v)}
                    />
                    <ContextInput
                      label="保留最近消息数"
                      value={
                        contextOverrides[r.key]?.keep_recent_count ??
                        effectiveContext(r.key).keep_recent_count
                      }
                      onChange={(v) => setContextOverride(r.key, 'keep_recent_count', v)}
                    />
                    <label className="flex items-center gap-2 py-1">
                      <span className="text-[10px] text-ink-500">mid-run compact</span>
                      <button
                        onClick={() =>
                          setContextOverride(
                            r.key,
                            'mid_run_compact',
                            !(
                              contextOverrides[r.key]?.mid_run_compact ??
                              effectiveContext(r.key).mid_run_compact
                            )
                          )
                        }
                        className={`relative w-8 h-4 rounded-full transition-colors ${
                          (contextOverrides[r.key]?.mid_run_compact ??
                          effectiveContext(r.key).mid_run_compact)
                            ? 'bg-ink-500'
                            : 'bg-ink-700'
                        }`}
                      >
                        <span
                          className={`absolute top-0.5 left-0.5 w-3 h-3 rounded-full bg-white transition-transform ${
                            (contextOverrides[r.key]?.mid_run_compact ??
                            effectiveContext(r.key).mid_run_compact)
                              ? 'translate-x-4'
                              : ''
                          }`}
                        />
                      </button>
                      <span className="text-[10px] text-ink-400">
                        {(contextOverrides[r.key]?.mid_run_compact ??
                        effectiveContext(r.key).mid_run_compact)
                          ? '开启'
                          : '关闭'}
                      </span>
                    </label>
                  </>
                )}
              </div>
            )}
          </div>
        );
      })}
      {/* ── 恢复默认按钮 ── */}
      <button
        onClick={async () => {
          try {
            const { resetContextConfig } = await import('../../api');
            await resetContextConfig();
            setSavedMsg('上下文配置已恢复默认');
            setTimeout(() => setSavedMsg(''), 2000);
          } catch (e) {
            setSavedMsg(String(e));
          }
        }}
        className="text-[10px] px-2 py-1 mt-1 text-ink-400 hover:text-ink-200 border border-ink-700 hover:border-ink-500 rounded transition-colors"
      >
        恢复默认
      </button>
    </div>
  );
}

function ContextInput({
  label,
  value,
  onChange,
}: {
  label: string;
  value: number;
  onChange: (value: number) => void;
}) {
  return (
    <label className="block">
      <span className="text-[10px] text-ink-500">{label}</span>
      <input
        type="number"
        min={0}
        value={value}
        onChange={(e) => onChange(Number(e.target.value))}
        className="w-full mt-0.5 px-2 py-1 text-xs bg-ink-800 border border-ink-700 rounded text-ink-200 focus:outline-none focus:border-ink-500"
      />
    </label>
  );
}
