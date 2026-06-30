import { useState, useEffect } from 'react';
import { useTranslation } from 'react-i18next';
import { ALL_ROLES } from '../../constants';
import { EFFORT_LABELS, EFFORT_ORDER, ROLE_BUILTIN_EFFORT } from '../../constants/reasoning';
import type { ReasoningEffort, RoleReasoningConfig } from '../../types';
import { getReasoningConfig, setReasoningConfig } from '../../api';
import {
  SettingsSection,
  SettingsChip,
  SettingsToggle,
  SettingsNumberField,
  SettingsMuted,
  SettingsAccordion,
  SettingsCheckbox,
  SettingsHint,
  SettingsAction,
} from './SettingsPrimitives';

interface ReasoningSettingsTabProps {
  expandedRole: string | null;
  setExpandedRole: (key: string | null) => void;
}

export default function ReasoningSettingsTab({
  expandedRole,
  setExpandedRole,
}: ReasoningSettingsTabProps) {
  const { t } = useTranslation();
  const [loading, setLoading] = useState(true);

  // Global state
  const [enabled, setEnabled] = useState(true);
  const [effort, setEffort] = useState<ReasoningEffort>('medium');
  const [budgetTokens, setBudgetTokens] = useState(0);

  // Per-role overrides
  const [roleOverrides, setRoleOverrides] = useState<Record<string, RoleReasoningConfig>>({});
  const [roleUseDefault, setRoleUseDefault] = useState<Record<string, boolean>>({});

  const roleList = ALL_ROLES.filter((r) => r.key !== 'default');

  useEffect(() => {
    getReasoningConfig()
      .then((cfg) => {
        setEnabled(cfg.enabled);
        setEffort(cfg.effort);
        setBudgetTokens(cfg.budget_tokens);

        const overrides: Record<string, RoleReasoningConfig> = {};
        const useDefault: Record<string, boolean> = {};
        for (const role of roleList) {
          if (cfg.roles[role.label]) {
            overrides[role.label] = cfg.roles[role.label];
            useDefault[role.key] = false;
          } else {
            useDefault[role.key] = true;
          }
        }
        setRoleOverrides(overrides);
        setRoleUseDefault(useDefault);
      })
      .catch(() => {})
      .finally(() => setLoading(false));
  }, []);

  const toggleRoleDefault = (roleKey: string, roleLabel: string) => {
    setRoleUseDefault((prev) => {
      const current = prev[roleKey] ?? true;
      if (!current) {
        // Switching to default — remove override
        setRoleOverrides((o) => {
          const next = { ...o };
          delete next[roleLabel];
          return next;
        });
      }
      return { ...prev, [roleKey]: !current };
    });
  };

  const setRoleEffort = (roleLabel: string, e: ReasoningEffort) => {
    setRoleOverrides((prev) => ({
      ...prev,
      [roleLabel]: { ...prev[roleLabel], effort: e },
    }));
  };

  const effectiveEffort = (roleKey: string, roleLabel: string): ReasoningEffort => {
    if (!(roleUseDefault[roleKey] ?? true) && roleOverrides[roleLabel]?.effort) {
      return roleOverrides[roleLabel]!.effort!;
    }
    return ROLE_BUILTIN_EFFORT[roleLabel] ?? effort;
  };

  if (loading) {
    return (
      <SettingsSection title="思考模式">
        <SettingsMuted>{t('common.loading')}</SettingsMuted>
      </SettingsSection>
    );
  }

  return (
    <SettingsSection
      title="思考模式"
      description="控制 LLM 推理/思考模式的强度。不同部门可使用不同策略以平衡成本与质量。"
    >
      {/* ── Global toggle ── */}
      <SettingsToggle label="启用思考模式" checked={enabled} onChange={setEnabled} />

      {/* ── Global effort ── */}
      {enabled && (
        <div className="space-y-1.5">
          <span className="text-xs font-medium text-ink-700">全局强度</span>
          <div className="flex gap-1.5 flex-wrap">
            {EFFORT_ORDER.map((e) => (
              <SettingsChip key={e} selected={effort === e} onClick={() => setEffort(e)}>
                {EFFORT_LABELS[e].zh}
              </SettingsChip>
            ))}
          </div>
          <p className="text-xs text-ink-600">{EFFORT_LABELS[effort].desc}</p>
        </div>
      )}

      {/* ── Anthropic budget tokens ── */}
      {enabled && effort !== 'none' && (
        <SettingsNumberField
          label="思考预算 (Anthropic budget_tokens)"
          value={budgetTokens}
          onChange={setBudgetTokens}
          min={0}
        />
      )}
      {enabled && effort !== 'none' && (
        <SettingsHint>
          0 = 使用模型默认值。仅对 Anthropic API 的 extended thinking 有效。
        </SettingsHint>
      )}

      {/* ── Per-role overrides ── */}
      <div className="pt-4 space-y-2">
        <h4 className="text-xs font-semibold text-ink-900">部门覆盖</h4>
        {roleList.map((r) => {
          const isExpanded = expandedRole === r.key;
          const usingDefault = roleUseDefault[r.key] ?? true;
          const eff = effectiveEffort(r.key, r.label);
          const effLabel = EFFORT_LABELS[eff];
          return (
            <SettingsAccordion
              key={r.key}
              expanded={isExpanded}
              onToggle={() => setExpandedRole(isExpanded ? null : r.key)}
              title={r.label}
              meta={effLabel.zh}
              leading={
                <SettingsCheckbox
                  checked={usingDefault}
                  onChange={() => toggleRoleDefault(r.key, r.label)}
                  label={t('common.useDefault')}
                  onClick={(e) => e.stopPropagation()}
                />
              }
            >
              {usingDefault ? (
                <SettingsMuted>
                  {t('settings.default')}（{effLabel.zh} — {effLabel.desc}）
                </SettingsMuted>
              ) : (
                <div className="space-y-2">
                  <div className="flex gap-1.5 flex-wrap">
                    {EFFORT_ORDER.map((e) => (
                      <SettingsChip
                        key={e}
                        size="sm"
                        selected={roleOverrides[r.label]?.effort === e}
                        onClick={() => setRoleEffort(r.label, e)}
                      >
                        {EFFORT_LABELS[e].zh}
                      </SettingsChip>
                    ))}
                  </div>
                  <p className="text-xs text-ink-600">
                    {EFFORT_LABELS[roleOverrides[r.label]?.effort ?? eff].desc}
                  </p>
                </div>
              )}
            </SettingsAccordion>
          );
        })}
      </div>

      {/* ── Save button ── */}
      <SettingsAction
        variant="accent"
        onClick={async () => {
          const roles: Record<string, RoleReasoningConfig> = {};
          for (const role of roleList) {
            if (!(roleUseDefault[role.key] ?? true) && roleOverrides[role.label]) {
              roles[role.label] = roleOverrides[role.label];
            }
          }
          await setReasoningConfig({ enabled, effort, budget_tokens: budgetTokens, roles });
        }}
      >
        保存思考配置
      </SettingsAction>
      <SettingsHint>此配置独立保存到 config.local.toml，不影响 API 密钥设置。</SettingsHint>
    </SettingsSection>
  );
}
