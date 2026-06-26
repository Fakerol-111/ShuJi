import { useState, useEffect, useCallback } from 'react';
import { useNavigate } from 'react-router-dom';
import { useTranslation } from 'react-i18next';
import { formatError } from '../utils/error';
import {
  getConfig,
  saveConfig,
  getContextConfig,
  saveContextConfig,
  checkApiConnection,
  getWorkflowPreset as apiGetPreset,
  setWorkflowPreset as apiSetPreset,
  getModelPreset,
  setModelPreset,
  getApprovalConfig,
  setApprovalConfig,
  getWorkflowConfig as apiGetWorkflowConfig,
  setWorkflowConfig as apiSetWorkflowConfig,
} from '../api';
import { ALL_ROLES } from '../constants';
import type {
  RoleEndpoint,
  ContextWindowConfig,
  RoleContextConfig,
  WorkflowConfig as WFConfig,
  ApprovalMode,
} from '../types';
import SettingsSidebar from '../components/settings/SettingsSidebar';
import ServiceConfigTab from '../components/settings/ServiceConfigTab';
import ContextSettingsTab from '../components/settings/ContextSettingsTab';
import SoulSettingsTab from '../components/settings/SoulSettingsTab';
import AppearanceTab from '../components/settings/AppearanceTab';
import { SettingsSaveButton } from '../components/settings/SettingsPrimitives';

export type SettingsCategory = 'service' | 'context' | 'soul' | 'appearance';

type RoleFormState = Pick<RoleEndpoint, 'api_key' | 'api_url' | 'model'>;
const DEFAULT_EMPTY: RoleFormState = { api_key: '', api_url: '', model: '' };

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

function initRoleConfigs(cfg: Record<string, RoleEndpoint>) {
  const def = cfg.default ?? DEFAULT_EMPTY;
  const overrides: Record<string, RoleFormState> = {};
  const useDefault: Record<string, boolean> = {};
  for (const role of ALL_ROLES) {
    if (role.key === 'default') continue;
    if (cfg[role.key]) {
      overrides[role.key] = cfg[role.key];
      useDefault[role.key] = false;
    } else {
      useDefault[role.key] = true;
    }
  }
  return { defaultCfg: def, overrides, useDefault };
}

interface SettingsPageProps {
  onClose?: () => void;
}

export default function SettingsPage({ onClose }: SettingsPageProps = {}) {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const [activeCategory, setActiveCategory] = useState<SettingsCategory>('service');

  // API config state
  const [defaultCfg, setDefaultCfg] = useState<RoleFormState>(DEFAULT_EMPTY);
  const [overrides, setOverrides] = useState<Record<string, RoleFormState>>({});
  const [useDefault, setUseDefault] = useState<Record<string, boolean>>({});
  const [expandedRole, setExpandedRole] = useState<string | null>(null);

  // Workflow / model preset state
  const [workflowPreset, setWorkflowPresetLocal] = useState('standard');
  const [workflowIntent, setWorkflowIntent] = useState<string>('auto');
  const [modelPreset, setModelPresetLocal] = useState('balanced');

  // Approval mode state
  const [approvalMode, setApprovalMode] = useState<ApprovalMode>('manual');
  const [approvalAutoRetries, setApprovalAutoRetries] = useState(3);

  // Context config state
  const [contextOverrides, setContextOverrides] = useState<Record<string, ContextRoleForm>>({});
  const [contextUseDefault, setContextUseDefault] = useState<Record<string, boolean>>({});

  // UI state
  const [savedMsg, setSavedMsg] = useState('');
  const [healthStatus, setHealthStatus] = useState<'idle' | 'checking' | 'ok' | 'fail'>('idle');
  const [healthMsg, setHealthMsg] = useState('');

  const loadConfig = useCallback(() => {
    getConfig()
      .then((cfg) => {
        const { defaultCfg: d, overrides: o, useDefault: u } = initRoleConfigs(cfg.roles ?? {});
        setDefaultCfg(d);
        setOverrides(o);
        setUseDefault(u);
      })
      .catch((e) => console.error(formatError(e)));
  }, []);

  const loadContextConfig = useCallback(() => {
    getContextConfig()
      .then((ctxCfg: ContextWindowConfig) => {
        const overrides: Record<string, ContextRoleForm> = {};
        const useDefault: Record<string, boolean> = {};
        const roles = ctxCfg.roles ?? {};
        for (const role of ALL_ROLES) {
          if (role.key === 'default') continue;
          if (roles[role.label]) {
            const raw = roles[role.label] as RoleContextConfig & { char_threshold?: number };
            overrides[role.key] = {
              token_threshold:
                raw.token_threshold ?? raw.char_threshold ?? DEFAULT_CONTEXT_VALUES.token_threshold,
              keep_recent_count: raw.keep_recent_count ?? DEFAULT_CONTEXT_VALUES.keep_recent_count,
              mid_run_compact: raw.mid_run_compact ?? DEFAULT_CONTEXT_VALUES.mid_run_compact,
            };
            useDefault[role.key] = false;
          } else {
            useDefault[role.key] = true;
          }
        }
        setContextOverrides(overrides);
        setContextUseDefault(useDefault);
      })
      .catch((e) => console.error(formatError(e)));
  }, []);

  const loadWorkflowConfig = useCallback(() => {
    apiGetWorkflowConfig()
      .then((cfg: WFConfig) => {
        setWorkflowIntent(cfg.intent);
        setWorkflowPresetLocal(cfg.governance);
      })
      .catch(() => {
        setWorkflowIntent('auto');
        apiGetPreset()
          .then(setWorkflowPresetLocal)
          .catch(() => setWorkflowPresetLocal('standard'));
      });
  }, []);

  const loadModelPreset = useCallback(() => {
    getModelPreset()
      .then(setModelPresetLocal)
      .catch(() => setModelPresetLocal('balanced'));
  }, []);

  const loadApprovalConfig = useCallback(() => {
    getApprovalConfig()
      .then((cfg) => {
        setApprovalMode(cfg.mode);
        setApprovalAutoRetries(cfg.auto_retries);
      })
      .catch(() => {
        setApprovalMode('manual');
        setApprovalAutoRetries(3);
      });
  }, []);

  useEffect(() => {
    loadConfig();
    loadContextConfig();
    loadWorkflowConfig();
    loadModelPreset();
    loadApprovalConfig();
  }, [loadConfig, loadContextConfig, loadWorkflowConfig, loadModelPreset, loadApprovalConfig]);

  const setOverride = (role: string, field: keyof RoleFormState, value: string) => {
    setOverrides((prev) => ({
      ...prev,
      [role]: { ...(prev[role] ?? defaultCfg), [field]: value },
    }));
    setModelPresetLocal('custom');
  };

  const toggleDefault = (role: string) => {
    setUseDefault((prev) => {
      const current = prev[role] ?? true;
      if (current) setOverrides((o) => ({ ...o, [role]: { ...defaultCfg } }));
      return { ...prev, [role]: !current };
    });
  };

  const toggleContextDefault = (roleKey: string) => {
    setContextUseDefault((prev) => {
      const current = prev[roleKey] ?? true;
      if (current) {
        setContextOverrides((o) => ({ ...o, [roleKey]: { ...DEFAULT_CONTEXT_VALUES } }));
      }
      return { ...prev, [roleKey]: !current };
    });
  };

  const setContextOverride = (role: string, field: string, value: number | boolean) => {
    setContextOverrides((prev) => ({
      ...prev,
      [role]: { ...(prev[role] ?? DEFAULT_CONTEXT_VALUES), [field]: value },
    }));
  };

  const applyDefaultToAll = () => {
    const allDefault: Record<string, boolean> = {};
    for (const r of ALL_ROLES) {
      if (r.key !== 'default') allDefault[r.key] = true;
    }
    setUseDefault(allDefault);
    setModelPresetLocal('custom');
  };

  const applyRoleToOthers = (sourceRole: string) => {
    const source = overrides[sourceRole];
    if (!source) return;
    const newOverrides = { ...overrides };
    const newUseDefault = { ...useDefault };
    for (const r of ALL_ROLES) {
      if (r.key === 'default' || r.key === sourceRole) continue;
      newOverrides[r.key] = { ...source };
      newUseDefault[r.key] = false;
    }
    setOverrides(newOverrides);
    setUseDefault(newUseDefault);
    setModelPresetLocal('custom');
  };

  const handleSave = async () => {
    try {
      const roles: Record<string, RoleEndpoint> = { default: defaultCfg };
      for (const role of ALL_ROLES) {
        if (role.key === 'default') continue;
        if (!(useDefault[role.key] ?? true)) roles[role.key] = overrides[role.key] ?? defaultCfg;
      }
      await saveConfig({ roles });
      await setModelPreset(modelPreset).catch((e) => {
        console.error('设置模型预设失败', e);
        setHealthStatus('fail');
        setHealthMsg(`${t('common.error')}: ${formatError(e)}`);
      });

      const ctxRoles: Record<string, ContextRoleForm> = {};
      for (const role of ALL_ROLES) {
        if (role.key === 'default') continue;
        if (!(contextUseDefault[role.key] ?? true)) {
          ctxRoles[role.label] = contextOverrides[role.key] ?? {
            token_threshold: 750_000,
            keep_recent_count: 24,
            mid_run_compact: false,
          };
        }
      }
      await saveContextConfig({ roles: ctxRoles });
      await apiSetPreset(workflowPreset);
      await apiSetWorkflowConfig({
        intent: workflowIntent as WFConfig['intent'],
        governance: workflowPreset as WFConfig['governance'],
        intent_override: null,
      });
      await setApprovalConfig({
        mode: approvalMode,
        auto_retries: approvalAutoRetries,
      });

      setSavedMsg(t('common.saved'));
      setTimeout(() => setSavedMsg(''), 2000);

      setHealthStatus('checking');
      setHealthMsg('');
      try {
        const def = defaultCfg;
        if (def.api_key && def.api_url && def.model) {
          await checkApiConnection(def.api_key, def.api_url, def.model);
          setHealthStatus('ok');
          setHealthMsg('连接成功');
        } else {
          setHealthStatus('idle');
          setHealthMsg('');
        }
      } catch (e) {
        setHealthStatus('fail');
        setHealthMsg(String(e));
      }
    } catch (e) {
      setSavedMsg(String(e));
    }
  };

  const roleList = ALL_ROLES.filter((r) => r.key !== 'default');

  const renderContent = () => {
    switch (activeCategory) {
      case 'service':
        return (
          <ServiceConfigTab
            defaultCfg={defaultCfg}
            setDefaultCfg={setDefaultCfg}
            overrides={overrides}
            setOverride={setOverride}
            useDefault={useDefault}
            toggleDefault={toggleDefault}
            expandedRole={expandedRole}
            setExpandedRole={setExpandedRole}
            roleList={roleList}
            onApplyDefaultToAll={applyDefaultToAll}
            onApplyRoleToOthers={applyRoleToOthers}
            workflowIntent={workflowIntent}
            setWorkflowIntent={setWorkflowIntent}
            workflowPreset={workflowPreset}
            setWorkflowPresetLocal={setWorkflowPresetLocal}
            modelPreset={modelPreset}
            setModelPresetLocal={setModelPresetLocal}
            approvalMode={approvalMode}
            setApprovalMode={setApprovalMode}
            approvalAutoRetries={approvalAutoRetries}
            setApprovalAutoRetries={setApprovalAutoRetries}
          />
        );
      case 'context':
        return (
          <ContextSettingsTab
            contextOverrides={contextOverrides}
            contextUseDefault={contextUseDefault}
            toggleContextDefault={toggleContextDefault}
            setContextOverride={setContextOverride}
            expandedRole={expandedRole}
            setExpandedRole={setExpandedRole}
            savedMsg={savedMsg}
            setSavedMsg={setSavedMsg}
          />
        );
      case 'soul':
        return <SoulSettingsTab setSavedMsg={setSavedMsg} />;
      case 'appearance':
        return <AppearanceTab />;
    }
  };

  const healthDisplay = () => {
    if (healthStatus === 'idle') return null;
    if (healthStatus === 'checking')
      return <span className="text-xs text-ink-300">{t('common.loading')}</span>;
    if (healthStatus === 'ok')
      return <span className="text-xs text-jade-light">{t('setup.connectionSuccess')}</span>;
    return (
      <span className="text-xs text-vermillion-light">
        {t('setup.connectionFailed')}: {healthMsg}
      </span>
    );
  };

  return (
    <div className="h-screen bg-surface-paper flex flex-col">
      {/* ── Header ── */}
      <header className="bg-ink-900 border-b border-gold/30 shrink-0 h-12 px-4 flex items-center justify-between">
        <div className="flex items-center gap-3">
          <button
            onClick={() => (onClose ? onClose() : navigate('/project'))}
            className="text-sm text-ink-300 hover:text-ink-50 transition-colors"
          >
            ← {onClose ? t('common.close') : t('settings.backToProject')}
          </button>
          <h1 className="font-display text-base font-semibold text-ink-50">
            {t('settings.title')}
          </h1>
        </div>
      </header>

      {/* ── Body: Sidebar + Content ── */}
      <div className="flex-1 flex min-h-0 overflow-hidden">
        <SettingsSidebar activeCategory={activeCategory} onSelect={setActiveCategory} />
        <main className="flex-1 overflow-y-auto bg-surface-paper">
          <div className="max-w-2xl mx-auto px-6 py-8">
            <header className="mb-8 pb-4 border-b border-border-fold">
              <h2 className="font-display text-title font-semibold text-ink-900">
                {t(
                  'settings.' +
                    (activeCategory === 'context'
                      ? 'contextWindow'
                      : activeCategory === 'soul'
                        ? 'soulManagement'
                        : activeCategory === 'service'
                          ? 'serviceConfig'
                          : activeCategory)
                )}
              </h2>
            </header>
            {renderContent()}
          </div>
        </main>
      </div>

      {/* ── Footer bar ── */}
      <div className="bg-ink-900 border-t border-ink-700 shrink-0 h-12 px-4 flex items-center justify-between">
        <div className="flex items-center gap-3">
          <SettingsSaveButton onClick={handleSave}>{t('common.saveAll')}</SettingsSaveButton>
          {savedMsg && (
            <span
              className={`text-sm ${savedMsg === t('common.saved') ? 'text-jade-light' : 'text-vermillion-light'}`}
            >
              {savedMsg}
            </span>
          )}
        </div>
        <div>{healthDisplay()}</div>
      </div>
    </div>
  );
}
