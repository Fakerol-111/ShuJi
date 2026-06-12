import { useState } from 'react';
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
  getWorkflowConfig as apiGetWorkflowConfig,
  setWorkflowConfig as apiSetWorkflowConfig,
} from '../api';
import {
  ALL_ROLES,
  CODE_THEMES,
  getCodeTheme,
  setCodeTheme as persistCodeTheme,
  FONT_SIZE_TIERS,
  getFontSize,
  setFontSize as persistFontSize,
} from '../constants';
import type {
  RoleEndpoint,
  ContextWindowConfig,
  RoleContextConfig,
  WorkflowConfig as WFConfig,
} from '../types';
import ApiSettingsTab from './settings/ApiSettingsTab';
import ContextSettingsTab from './settings/ContextSettingsTab';
import WorkflowSettingsTab from './settings/WorkflowSettingsTab';
import SoulSettingsTab from './settings/SoulSettingsTab';

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

interface SettingsMenuProps {
  open: boolean;
  setOpen: (open: boolean) => void;
}

export default function SettingsMenu({ open, setOpen }: SettingsMenuProps) {
  const [defaultCfg, setDefaultCfg] = useState<RoleFormState>(DEFAULT_EMPTY);
  const [overrides, setOverrides] = useState<Record<string, RoleFormState>>({});
  const [useDefault, setUseDefault] = useState<Record<string, boolean>>({});
  const [expandedRole, setExpandedRole] = useState<string | null>(null);
  const [savedMsg, setSavedMsg] = useState('');
  const [healthStatus, setHealthStatus] = useState<'idle' | 'checking' | 'ok' | 'fail'>('idle');
  const [healthMsg, setHealthMsg] = useState('');
  const [workflowPreset, setWorkflowPresetLocal] = useState('standard');
  const [workflowIntent, setWorkflowIntent] = useState<string>('auto');
  const [modelPreset, setModelPresetLocal] = useState('balanced');
  const [codeTheme, setCodeThemeLocal] = useState(getCodeTheme);
  const [fontSize, setFontSizeLocal] = useState(getFontSize);
  const [contextOverrides, setContextOverrides] = useState<Record<string, ContextRoleForm>>({});
  const [contextUseDefault, setContextUseDefault] = useState<Record<string, boolean>>({});

  const loadConfig = () => {
    getConfig()
      .then((cfg) => {
        const { defaultCfg: d, overrides: o, useDefault: u } = initRoleConfigs(cfg.roles ?? {});
        setDefaultCfg(d);
        setOverrides(o);
        setUseDefault(u);
      })
      .catch((e) => console.error(formatError(e)));
  };
  const loadContextConfig = () => {
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
  };

  const loadWorkflowConfig = () => {
    apiGetWorkflowConfig()
      .then((cfg: WFConfig) => {
        setWorkflowIntent(cfg.intent);
        setWorkflowPresetLocal(cfg.governance);
      })
      .catch((e) => {
        console.error('加载工作流配置失败', e);
        setWorkflowIntent('auto');
        apiGetPreset()
          .then(setWorkflowPresetLocal)
          .catch((e2) => {
            console.error('加载工作流预设失败', e2);
            setWorkflowPresetLocal('standard');
          });
      });
  };

  const loadModelPreset = () => {
    getModelPreset()
      .then(setModelPresetLocal)
      .catch((e) => {
        console.error('加载模型预设失败', e);
        setModelPresetLocal('balanced');
      });
  };

  const toggle = () => {
    if (!open) {
      loadConfig();
      loadContextConfig();
      loadWorkflowConfig();
      loadModelPreset();
    }
    setOpen(!open);
  };

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
        const role = ALL_ROLES.find((r) => r.key === roleKey);
        const preset = role
          ? { token_threshold: 750_000, keep_recent_count: 24, mid_run_compact: false }
          : undefined;
        setContextOverrides((o) => ({ ...o, [roleKey]: preset ?? { ...DEFAULT_CONTEXT_VALUES } }));
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
        setHealthMsg(`设置预设失败: ${formatError(e)}`);
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

      setSavedMsg('已保存');
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

  return (
    <div className="relative">
      <button
        onClick={toggle}
        className="text-xs px-2 py-1 text-ink-400 hover:text-ink-100 hover:bg-ink-800 rounded"
      >
        ⚙ 设置
      </button>

      {open && (
        <div className="absolute right-0 top-full mt-1 w-[360px] bg-ink-900 border border-ink-700 rounded-lg shadow-xl z-50 p-3 space-y-3 max-h-[80vh] overflow-y-auto">
          <ApiSettingsTab
            defaultCfg={defaultCfg}
            setDefaultCfg={setDefaultCfg}
            overrides={overrides}
            setOverride={setOverride}
            useDefault={useDefault}
            toggleDefault={toggleDefault}
            expandedRole={expandedRole}
            setExpandedRole={setExpandedRole}
            roleList={roleList}
          />

          <div className="pt-2 border-t border-ink-700">
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
          </div>

          <div className="pt-2 border-t border-ink-700">
            <WorkflowSettingsTab
              workflowIntent={workflowIntent}
              setWorkflowIntent={setWorkflowIntent}
              workflowPreset={workflowPreset}
              setWorkflowPresetLocal={setWorkflowPresetLocal}
              modelPreset={modelPreset}
              setModelPresetLocal={setModelPresetLocal}
            />
          </div>

          <div className="pt-2 border-t border-ink-700">
            <SoulSettingsTab setSavedMsg={setSavedMsg} />
          </div>

          {/* ── 字体大小 ── */}
          <div className="space-y-1 pt-2 border-t border-ink-700">
            <span className="text-[11px] font-semibold text-ink-300">字体大小</span>
            <div className="flex gap-1 flex-wrap">
              {Object.entries(FONT_SIZE_TIERS).map(([key, tier]) => (
                <button
                  key={key}
                  onClick={() => {
                    setFontSizeLocal(key);
                    persistFontSize(key);
                    document.documentElement.dataset.fontSize = key;
                  }}
                  className={`text-[10px] px-2 py-1 rounded-full border transition-colors ${fontSize === key ? 'bg-ink-700 text-ink-100 border-ink-600' : 'bg-ink-800 text-ink-400 border-ink-700 hover:border-ink-500'}`}
                  title={tier.description}
                >
                  {tier.label}
                </button>
              ))}
            </div>
          </div>

          {/* ── 代码主题 ── */}
          <div className="space-y-1 pt-2 border-t border-ink-700">
            <span className="text-[11px] font-semibold text-ink-300">代码主题</span>
            <div className="flex gap-1 flex-wrap">
              {Object.entries(CODE_THEMES).map(([key, theme]) => (
                <button
                  key={key}
                  onClick={() => {
                    setCodeThemeLocal(key);
                    persistCodeTheme(key);
                    document.documentElement.dataset.codeTheme = key;
                  }}
                  className={`text-[10px] px-2 py-1 rounded-full border transition-colors ${codeTheme === key ? 'bg-ink-700 text-ink-100 border-ink-600' : 'bg-ink-800 text-ink-400 border-ink-700 hover:border-ink-500'}`}
                >
                  {theme.label}
                </button>
              ))}
            </div>
          </div>

          {/* ── Save ── */}
          <div className="flex items-center gap-2 pt-1">
            <button
              onClick={handleSave}
              className="text-xs px-3 py-1.5 bg-ink-700 text-ink-200 rounded hover:bg-ink-600 transition-colors"
            >
              保存所有更改
            </button>
            {savedMsg && (
              <span
                className={`text-[10px] ${savedMsg === '已保存' ? 'text-green-400' : 'text-red-400'}`}
              >
                {savedMsg}
              </span>
            )}
          </div>

          {healthStatus !== 'idle' && (
            <div
              className={`text-[10px] px-2 py-1 rounded ${healthStatus === 'checking' ? 'text-ink-400 bg-ink-800' : healthStatus === 'ok' ? 'text-green-400 bg-green-900/20' : 'text-red-400 bg-red-900/20'}`}
            >
              {healthStatus === 'checking' && '⏳ 探测 API 连接中...'}
              {healthStatus === 'ok' && '✔ 连接成功'}
              {healthStatus === 'fail' && `✘ 连接失败: ${healthMsg}`}
            </div>
          )}
        </div>
      )}
    </div>
  );
}
