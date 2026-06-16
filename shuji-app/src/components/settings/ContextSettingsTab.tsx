import { useTranslation } from 'react-i18next';
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
  const { t } = useTranslation();
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
      title={t('settings.contextConfig')}
      description={`${t('common.descriptionFallback')}：${DEFAULT_CONTEXT_VALUES.token_threshold.toLocaleString()} tokens`}
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
                  label={t('common.useDefault')}
                  onClick={(e) => e.stopPropagation()}
                />
              }
            >
              {usingDefault ? (
                <SettingsMuted>
                  {t('settings.default')}（{effective.token_threshold.toLocaleString()} tokens，{t('settings.keepRecentMessages')}{' '}
                  {effective.keep_recent_count}）
                  <br />
                  {t('common.useDefault')}
                </SettingsMuted>
              ) : (
                <>
                  <SettingsNumberField
                    label={t('settings.compressionThreshold')}
                    value={contextOverrides[r.key]?.token_threshold ?? effective.token_threshold}
                    onChange={(v) => setContextOverride(r.key, 'token_threshold', v)}
                  />
                  <SettingsNumberField
                    label={t('settings.keepRecentMessages')}
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
            setSavedMsg(t('common.saved'));
            setTimeout(() => setSavedMsg(''), 2000);
          } catch (e) {
            setSavedMsg(String(e));
          }
        }}
      >
        {t('common.restoreDefault')}
      </SettingsAction>
      <SettingsHint>{t('common.saveAll')}</SettingsHint>
    </SettingsSection>
  );
}
