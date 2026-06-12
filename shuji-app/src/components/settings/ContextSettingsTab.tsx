import { ALL_ROLES, ROLE_CONTEXT_DEFAULTS } from '../../constants';
import {
  SettingsSection,
  SettingsNumberField,
  SettingsMuted,
  SettingsAccordion,
  SettingsCheckbox,
  SettingsToggle,
  SettingsAction,
  SettingsHint,
} from './SettingsPrimitives';

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
    <SettingsSection
      title="上下文窗口配置"
      description={`全局回退：${DEFAULT_CONTEXT_VALUES.token_threshold.toLocaleString()} tokens · cl100k · DeepSeek 1M 接近上限再压缩`}
    >
      <div className="space-y-2">
        {roleList.map((r) => {
          const isExpanded = expandedRole === r.key;
          const usingDefault = contextUseDefault[r.key] ?? true;
          const effective = effectiveContext(r.key);
          return (
            <SettingsAccordion
              key={r.key}
              expanded={isExpanded}
              onToggle={() => setExpandedRole(isExpanded ? null : r.key)}
              title={r.label}
              meta={`${effective.token_threshold.toLocaleString()} tokens`}
              leading={
                <SettingsCheckbox
                  checked={usingDefault}
                  onChange={() => toggleContextDefault(r.key)}
                  label="使用默认"
                  onClick={(e) => e.stopPropagation()}
                />
              }
            >
              {usingDefault ? (
                <SettingsMuted>
                  使用部门内置推荐值（{effective.token_threshold.toLocaleString()} tokens，保留{' '}
                  {effective.keep_recent_count} 条）
                  <br />
                  取消勾选「使用默认」可单独覆盖
                </SettingsMuted>
              ) : (
                <>
                  <SettingsNumberField
                    label="压缩阈值（tokens）"
                    value={contextOverrides[r.key]?.token_threshold ?? effective.token_threshold}
                    onChange={(v) => setContextOverride(r.key, 'token_threshold', v)}
                  />
                  <SettingsNumberField
                    label="保留最近消息数"
                    value={
                      contextOverrides[r.key]?.keep_recent_count ?? effective.keep_recent_count
                    }
                    onChange={(v) => setContextOverride(r.key, 'keep_recent_count', v)}
                  />
                  <SettingsToggle
                    label="运行中压缩（mid-run compact）"
                    checked={contextOverrides[r.key]?.mid_run_compact ?? effective.mid_run_compact}
                    onChange={(v) => setContextOverride(r.key, 'mid_run_compact', v)}
                  />
                </>
              )}
            </SettingsAccordion>
          );
        })}
      </div>

      <SettingsAction
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
      >
        恢复默认
      </SettingsAction>
      <SettingsHint>修改后请点击页面底部的「保存所有更改」。</SettingsHint>
    </SettingsSection>
  );
}
