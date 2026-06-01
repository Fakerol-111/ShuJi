import { useState } from "react";
import { getConfig, saveConfig, getContextConfig, saveContextConfig, checkApiConnection, getWorkflowPreset as apiGetPreset, setWorkflowPreset as apiSetPreset, getModelPreset, setModelPreset } from "../api";
import { ALL_ROLES, CODE_THEMES, ROLE_CONTEXT_DEFAULTS, getCodeTheme, setCodeTheme as persistCodeTheme } from "../constants";
import type { RoleEndpoint, ContextWindowConfig, RoleContextConfig } from "../types";

// ── Provider presets (shared with SetupPage) ───────────────

const API_URL_PRESETS = [
  { label: "DeepSeek", url: "https://api.deepseek.com/chat/completions" },
  { label: "Anthropic", url: "https://api.anthropic.com/v1/messages" },
  { label: "OpenAI", url: "https://api.openai.com/v1/chat/completions" },
  { label: "自定义", url: "" },
];

const MODEL_PRESETS: Record<string, string[]> = {
  "https://api.deepseek.com/chat/completions": ["deepseek-v4-flash", "deepseek-4-pro"],
  "https://api.anthropic.com/v1/messages": ["claude-sonnet-4-20250514", "claude-haiku-4-5-20251001"],
  "https://api.openai.com/v1/chat/completions": ["gpt-4o", "gpt-4o-mini"],
};

function detectProvider(url: string): string {
  if (url.includes("anthropic.com")) return "Anthropic";
  if (url.includes("deepseek.com")) return "DeepSeek";
  if (url.includes("openai.com")) return "OpenAI";
  return "自定义";
}

// ── Helpers ────────────────────────────────────────────────

type RoleFormState = Pick<RoleEndpoint, "api_key" | "api_url" | "model">;

const DEFAULT_EMPTY: RoleFormState = { api_key: "", api_url: "", model: "" };

// ── Context window config ──────────────────────────────────

interface ContextRoleForm {
  token_threshold: number;
  keep_recent_count: number;
  mid_run_compact: boolean;
}

/// Matches default values in config/mod.rs
const DEFAULT_CONTEXT_VALUES: ContextRoleForm = {
  token_threshold: 750_000,
  keep_recent_count: 24,
  mid_run_compact: false,
};

function initRoleConfigs(cfg: Record<string, RoleEndpoint>): {
  defaultCfg: RoleFormState;
  overrides: Record<string, RoleFormState>;
  useDefault: Record<string, boolean>;
} {
  const def = cfg.default ?? DEFAULT_EMPTY;
  const overrides: Record<string, RoleFormState> = {};
  const useDefault: Record<string, boolean> = {};
  for (const role of ALL_ROLES) {
    if (role.key === "default") continue;
    if (cfg[role.key]) {
      overrides[role.key] = cfg[role.key];
      useDefault[role.key] = false;
    } else {
      useDefault[role.key] = true;
    }
  }
  return { defaultCfg: def, overrides, useDefault };
}

// ── Component ──────────────────────────────────────────────

interface SettingsMenuProps {
  open: boolean;
  setOpen: (open: boolean) => void;
}

export default function SettingsMenu({ open, setOpen }: SettingsMenuProps) {
  const [defaultCfg, setDefaultCfg] = useState<RoleFormState>(DEFAULT_EMPTY);
  const [overrides, setOverrides] = useState<Record<string, RoleFormState>>({});
  const [useDefault, setUseDefault] = useState<Record<string, boolean>>({});
  const [expandedRole, setExpandedRole] = useState<string | null>(null);
  const [savedMsg, setSavedMsg] = useState("");
  const [healthStatus, setHealthStatus] = useState<"idle" | "checking" | "ok" | "fail">("idle");
  const [healthMsg, setHealthMsg] = useState("");
  const [workflowPreset, setWorkflowPresetLocal] = useState("standard");
  const [modelPreset, setModelPresetLocal] = useState("balanced");
  const [codeTheme, setCodeThemeLocal] = useState(getCodeTheme);

  // Context window config state
  const [contextOverrides, setContextOverrides] = useState<Record<string, ContextRoleForm>>({});
  const [contextUseDefault, setContextUseDefault] = useState<Record<string, boolean>>({});

  const loadConfig = () => {
    getConfig().then((cfg) => {
      const { defaultCfg: d, overrides: o, useDefault: u } = initRoleConfigs(cfg.roles ?? {});
      setDefaultCfg(d);
      setOverrides(o);
      setUseDefault(u);
    }).catch((e) => console.error("读取配置失败:", e));
  };

  const loadContextConfig = () => {
    getContextConfig().then((ctxCfg: ContextWindowConfig) => {
      const overrides: Record<string, ContextRoleForm> = {};
      const useDefault: Record<string, boolean> = {};
      const roles = ctxCfg.roles ?? {};
      for (const role of ALL_ROLES) {
        if (role.key === "default") continue;
        // 运行时 lookup 使用中文部门名（role.label）
        if (roles[role.label]) {
          const raw = roles[role.label] as RoleContextConfig & {
            char_threshold?: number;
          };
          overrides[role.key] = {
            token_threshold:
              raw.token_threshold ?? raw.char_threshold ?? DEFAULT_CONTEXT_VALUES.token_threshold,
            keep_recent_count:
              raw.keep_recent_count ?? DEFAULT_CONTEXT_VALUES.keep_recent_count,
            mid_run_compact:
              raw.mid_run_compact ?? DEFAULT_CONTEXT_VALUES.mid_run_compact,
          };
          useDefault[role.key] = false;
        } else {
          useDefault[role.key] = true;
        }
      }
      setContextOverrides(overrides);
      setContextUseDefault(useDefault);
    }).catch((e) => console.error("读取上下文配置失败:", e));
  };

  const loadWorkflowPreset = () => {
    apiGetPreset().then(setWorkflowPresetLocal).catch(() => setWorkflowPresetLocal("standard"));
  };

  const loadModelPreset = () => {
    getModelPreset().then(setModelPresetLocal).catch(() => setModelPresetLocal("balanced"));
  };

  const toggle = () => {
    if (!open) { loadConfig(); loadContextConfig(); loadWorkflowPreset(); loadModelPreset(); }
    setOpen(!open);
  };

  const setOverride = (role: string, field: keyof RoleFormState, value: string) => {
    setOverrides((prev) => ({ ...prev, [role]: { ...(prev[role] ?? defaultCfg), [field]: value } }));
    // Manual override → mark preset as custom
    setModelPresetLocal("custom");
  };

  const toggleDefault = (role: string) => {
    setUseDefault((prev) => {
      const current = prev[role] ?? true;
      if (current) {
        // Switching OFF "使用默认" — pre-fill with current default values
        setOverrides((o) => ({ ...o, [role]: { ...defaultCfg } }));
      }
      return { ...prev, [role]: !current };
    });
  };

  const toggleContextDefault = (roleKey: string) => {
    setContextUseDefault((prev) => {
      const current = prev[roleKey] ?? true;
      if (current) {
        const role = ALL_ROLES.find((r) => r.key === roleKey);
        const preset = role ? ROLE_CONTEXT_DEFAULTS[role.label] : undefined;
        setContextOverrides((o) => ({
          ...o,
          [roleKey]: preset
            ? {
                token_threshold: preset.token_threshold,
                keep_recent_count: preset.keep_recent_count,
                mid_run_compact: preset.mid_run_compact,
              }
            : { ...DEFAULT_CONTEXT_VALUES },
        }));
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

  const handleSave = async () => {
    try {
      // Save API config
      const roles: Record<string, RoleEndpoint> = { default: defaultCfg };
      for (const role of ALL_ROLES) {
        if (role.key === "default") continue;
        if (!(useDefault[role.key] ?? true)) {
          roles[role.key] = overrides[role.key] ?? defaultCfg;
        }
      }
      await saveConfig({ roles });
      // Apply model preset (updates per-role model fields)
      await setModelPreset(modelPreset).catch(() => {});

      // Save context window config
      const ctxRoles: Record<string, ContextRoleForm> = {};
      for (const role of ALL_ROLES) {
        if (role.key === "default") continue;
        if (!(contextUseDefault[role.key] ?? true)) {
          ctxRoles[role.label] = contextOverrides[role.key] ?? effectiveContext(role.key);
        }
      }
      await saveContextConfig({ roles: ctxRoles });

      // Save workflow preset
      await apiSetPreset(workflowPreset);

      setSavedMsg("已保存");
      setTimeout(() => setSavedMsg(""), 2000);

      // Probe default endpoint
      setHealthStatus("checking");
      setHealthMsg("");
      try {
        const def = effectiveRole("default");
        if (def.api_key && def.api_url && def.model) {
          await checkApiConnection(def.api_key, def.api_url, def.model);
          setHealthStatus("ok");
          setHealthMsg("连接成功");
        } else {
          setHealthStatus("idle");
          setHealthMsg("");
        }
      } catch (e) {
        setHealthStatus("fail");
        setHealthMsg(String(e));
      }
    } catch (e) {
      setSavedMsg(String(e));
    }
  };

  // Determine the effective values for a role (override or fallback to default)
  const effectiveRole = (key: string): RoleFormState =>
    !(useDefault[key] ?? true) && overrides[key] ? overrides[key] : defaultCfg;

  // ── Roles excluding "default" ──
  const roleList = ALL_ROLES.filter((r) => r.key !== "default");

  return (
    <div className="relative">
      <button onClick={toggle} className="text-xs px-2 py-1 text-ink-400 hover:text-ink-100 hover:bg-ink-800 rounded">
        ⚙ 设置
      </button>

      {open && (
        <div className="absolute right-0 top-full mt-1 w-[360px] bg-ink-900 border border-ink-700 rounded-lg shadow-xl z-50 p-3 space-y-3 max-h-[80vh] overflow-y-auto">
          {/* ── Default role ── */}
          <div className="space-y-1.5 pb-2 border-b border-ink-700">
            <span className="text-[11px] font-semibold text-ink-300">默认（全局）</span>
            <ConfigInput label="API 密钥" type="password" value={defaultCfg.api_key} onChange={(v) => setDefaultCfg({ ...defaultCfg, api_key: v })} />
            <ConfigInput label="API URL" value={defaultCfg.api_url} onChange={(v) => setDefaultCfg({ ...defaultCfg, api_url: v })} />
            <ModelSuggestions url={defaultCfg.api_url} model={defaultCfg.model} onSelect={(m) => setDefaultCfg({ ...defaultCfg, model: m })} />
            <div className="flex gap-1 flex-wrap mt-1">
              {API_URL_PRESETS.map((p) => (
                <button
                  key={p.label}
                  onClick={() => setDefaultCfg({ ...defaultCfg, api_url: p.url, model: MODEL_PRESETS[p.url]?.[0] ?? defaultCfg.model })}
                  className={`text-[10px] px-2 py-0.5 rounded-full border transition-colors ${
                    defaultCfg.api_url === p.url
                      ? "bg-ink-700 text-ink-100 border-ink-600"
                      : "bg-ink-800 text-ink-400 border-ink-700 hover:border-ink-500"
                  }`}
                >
                  {p.label}
                </button>
              ))}
            </div>
          </div>

          {/* ── Per-role overrides ── */}
          <div className="space-y-0.5">
            <span className="text-[11px] font-semibold text-ink-300">各角色覆盖</span>
            {roleList.map((r) => {
              const isExpanded = expandedRole === r.key;
              const usingDefault = useDefault[r.key] ?? true;
              const effective = effectiveRole(r.key);
              const provider = detectProvider(effective.api_url);
              return (
                <div key={r.key} className="border border-ink-800 rounded">
                  {/* Header row */}
                  <button
                    onClick={() => setExpandedRole(isExpanded ? null : r.key)}
                    className="w-full flex items-center gap-2 px-2 py-1.5 text-xs text-ink-300 hover:bg-ink-800 transition-colors"
                  >
                    <span className="text-ink-500 shrink-0">{isExpanded ? "▾" : "▸"}</span>
                    <label className="flex items-center gap-1.5 shrink-0" onClick={(e) => e.stopPropagation()}>
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

                  {/* Expanded fields */}
                  {isExpanded && (
                    <div className="px-2 pb-2 space-y-1">
                      {usingDefault ? (
                        <div className="text-[10px] text-ink-500 italic px-1 py-2">
                          使用默认配置（{defaultCfg.model || "未设置"}）
                          <br />
                          取消勾选"使用默认"可单独设置
                        </div>
                      ) : (
                        <>
                          <ConfigInput label="API 密钥" type="password" value={overrides[r.key]?.api_key ?? ""} onChange={(v) => setOverride(r.key, "api_key", v)} />
                          <ConfigInput label="API URL" value={overrides[r.key]?.api_url ?? ""} onChange={(v) => setOverride(r.key, "api_url", v)} />
                          <ModelSuggestions url={overrides[r.key]?.api_url ?? ""} model={overrides[r.key]?.model ?? ""} onSelect={(m) => setOverride(r.key, "model", m)} />
                        </>
                      )}
                    </div>
                  )}
                </div>
              );
            })}
          </div>

          {/* ── Context window config ── */}
          <div className="space-y-0.5 pt-2 border-t border-ink-700">
            <span className="text-[11px] font-semibold text-ink-300">上下文窗口配置</span>
            <div className="text-[10px] text-ink-500 px-1 pb-1">
              全局回退: {DEFAULT_CONTEXT_VALUES.token_threshold.toLocaleString()} tokens · cl100k · DeepSeek 1M 接近上限再压缩
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
                    <span className="text-ink-500 shrink-0">{isExpanded ? "▾" : "▸"}</span>
                    <label className="flex items-center gap-1.5 shrink-0" onClick={(e) => e.stopPropagation()}>
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
                          使用部门内置推荐值（{effectiveContext(r.key).token_threshold.toLocaleString()} tokens，保留 {effectiveContext(r.key).keep_recent_count} 条）
                          <br />
                          取消勾选"使用默认"可单独覆盖
                        </div>
                      ) : (
                        <>
                          <ContextInput label="压缩阈值（tokens）" value={contextOverrides[r.key]?.token_threshold ?? effectiveContext(r.key).token_threshold} onChange={(v) => setContextOverride(r.key, "token_threshold", v)} />
                          <ContextInput label="保留最近消息数" value={contextOverrides[r.key]?.keep_recent_count ?? effectiveContext(r.key).keep_recent_count} onChange={(v) => setContextOverride(r.key, "keep_recent_count", v)} />
                          <label className="flex items-center gap-2 py-1">
                            <span className="text-[10px] text-ink-500">mid-run compact</span>
                            <button
                              onClick={() => setContextOverride(r.key, "mid_run_compact", !(contextOverrides[r.key]?.mid_run_compact ?? effectiveContext(r.key).mid_run_compact))}
                              className={`relative w-8 h-4 rounded-full transition-colors ${
                                (contextOverrides[r.key]?.mid_run_compact ?? effectiveContext(r.key).mid_run_compact) ? "bg-ink-500" : "bg-ink-700"
                              }`}
                            >
                              <span className={`absolute top-0.5 left-0.5 w-3 h-3 rounded-full bg-white transition-transform ${
                                (contextOverrides[r.key]?.mid_run_compact ?? effectiveContext(r.key).mid_run_compact) ? "translate-x-4" : ""
                              }`} />
                            </button>
                            <span className="text-[10px] text-ink-400">
                              {(contextOverrides[r.key]?.mid_run_compact ?? effectiveContext(r.key).mid_run_compact) ? "开启" : "关闭"}
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
                  const { resetContextConfig } = await import("../api");
                  await resetContextConfig();
                  setContextOverrides({});
                  setContextUseDefault({});
                  setSavedMsg("上下文配置已恢复默认");
                  setTimeout(() => setSavedMsg(""), 2000);
                } catch (e) {
                  setSavedMsg(String(e));
                }
              }}
              className="text-[10px] px-2 py-1 mt-1 text-ink-400 hover:text-ink-200 border border-ink-700 hover:border-ink-500 rounded transition-colors"
            >
              恢复默认
            </button>
          </div>

          {/* ── Soul 管理 ── */}
          <div className="space-y-1 pt-2 border-t border-ink-700">
            <span className="text-[11px] font-semibold text-ink-300">Soul 管理</span>
            <div className="flex gap-2 flex-wrap pt-1">
              <button
                onClick={async () => {
                  try {
                    const { getSoulContent } = await import("../api");
                    const content = await getSoulContent();
                    if (!content) {
                      setSavedMsg("soul 为空或不存在");
                      setTimeout(() => setSavedMsg(""), 2000);
                      return;
                    }
                    await navigator.clipboard.writeText(content);
                    setSavedMsg("soul 已复制到剪贴板");
                    setTimeout(() => setSavedMsg(""), 2000);
                  } catch (e) {
                    setSavedMsg(String(e));
                  }
                }}
                className="text-[10px] px-2 py-1 text-ink-400 hover:text-ink-200 border border-ink-700 hover:border-ink-500 rounded transition-colors"
              >
                导出 soul（复制）
              </button>
              <button
                onClick={async () => {
                  try {
                    const { clearSoul } = await import("../api");
                    await clearSoul();
                    setSavedMsg("soul 已重置为默认");
                    setTimeout(() => setSavedMsg(""), 2000);
                  } catch (e) {
                    setSavedMsg(String(e));
                  }
                }}
                className="text-[10px] px-2 py-1 text-red-400 hover:text-red-300 border border-red-800 hover:border-red-600 rounded transition-colors"
              >
                清空 soul
              </button>
            </div>
            <div className="text-[10px] text-ink-500 px-1">
              soul 超 8KB 时将自动压缩。单条经验/教训/偏好 ≤500 字符。
            </div>
          </div>

          {/* ── Workflow preset ── */}
          <div className="space-y-1 pt-2 border-t border-ink-700">
            <span className="text-[11px] font-semibold text-ink-300">流程预设</span>
            <div className="flex gap-1 flex-wrap">
              {(["full", "standard", "fast", "audit"] as const).map((p) => (
                <button
                  key={p}
                  onClick={() => setWorkflowPresetLocal(p)}
                  className={`text-[10px] px-2 py-1 rounded-full border transition-colors ${
                    workflowPreset === p
                      ? "bg-ink-700 text-ink-100 border-ink-600"
                      : "bg-ink-800 text-ink-400 border-ink-700 hover:border-ink-500"
                  }`}
                >
                  {{ full: "完整治理", standard: "标准", fast: "极速", audit: "审计" }[p]}
                </button>
              ))}
            </div>
            <div className="text-[10px] text-ink-500 px-1">
              {{
                full: "所有流程必经审查。适合高复杂度任务。",
                standard: "跳过门下审查。适合中等复杂度任务。（默认）",
                fast: "跳过设计/审查，直达执行。适合小改动。",
                audit: "强制审查和规范检查。适合合规场景。",
              }[workflowPreset]}
            </div>
          </div>

          {/* ── Model preset ── */}
          <div className="space-y-1 pt-2 border-t border-ink-700">
            <span className="text-[11px] font-semibold text-ink-300">模型分级预设</span>
            <div className="flex gap-1 flex-wrap items-center">
              {[
                { key: "balanced", label: "均衡" },
                { key: "economy", label: "经济" },
                { key: "quality", label: "质量" },
              ].map((p) => (
                <button
                  key={p.key}
                  onClick={() => { setModelPresetLocal(p.key); }}
                  className={`text-[10px] px-2 py-1 rounded-full border transition-colors ${
                    modelPreset === p.key
                      ? "bg-ink-700 text-ink-100 border-ink-600"
                      : "bg-ink-800 text-ink-400 border-ink-700 hover:border-ink-500"
                  }`}
                >
                  {p.label}
                </button>
              ))}
              {modelPreset === "custom" && (
                <span className="text-[10px] text-ink-400 italic px-1">自定义</span>
              )}
            </div>
            <div className="text-[10px] text-ink-500 px-1">
              {{
                balanced: "全部部门使用同一模型（默认）",
                economy: "审查/检查部门用轻量模型，设计/编码用默认",
                quality: "设计/编码部门用最强模型，其余用默认",
                custom: "已手动修改部门模型配置",
              }[modelPreset] || ""}
            </div>
            <div className="text-[10px] text-ink-400 px-1">
              切换预设会覆盖相关角色的 model 字段，不改 API URL/Key。
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
                  className={`text-[10px] px-2 py-1 rounded-full border transition-colors ${
                    codeTheme === key
                      ? "bg-ink-700 text-ink-100 border-ink-600"
                      : "bg-ink-800 text-ink-400 border-ink-700 hover:border-ink-500"
                  }`}
                >
                  {theme.label}
                </button>
              ))}
            </div>
          </div>

          {/* ── Save ── */}
          <div className="flex items-center gap-2 pt-1">
            <button onClick={handleSave} className="text-xs px-3 py-1.5 bg-ink-700 text-ink-200 rounded hover:bg-ink-600 transition-colors">
              保存所有更改
            </button>
            {savedMsg && (
              <span className={`text-[10px] ${savedMsg === "已保存" ? "text-green-400" : "text-red-400"}`}>
                {savedMsg}
              </span>
            )}
          </div>

          {/* ── Health check indicator ── */}
          {healthStatus !== "idle" && (
            <div className={`text-[10px] px-2 py-1 rounded ${
              healthStatus === "checking" ? "text-ink-400 bg-ink-800" :
              healthStatus === "ok" ? "text-green-400 bg-green-900/20" :
              "text-red-400 bg-red-900/20"
            }`}>
              {healthStatus === "checking" && "⏳ 探测 API 连接中..."}
              {healthStatus === "ok" && "✔ 连接成功"}
              {healthStatus === "fail" && `✘ 连接失败: ${healthMsg}`}
            </div>
          )}
        </div>
      )}
    </div>
  );
}

// ── Sub-components ─────────────────────────────────────────

function ConfigInput({ label, value, onChange, type = "text" }: { label: string; value: string; onChange: (value: string) => void; type?: string }) {
  return (
    <label className="block">
      <span className="text-[10px] text-ink-500">{label}</span>
      <input
        type={type}
        value={value}
        onChange={(e) => onChange(e.target.value)}
        className="w-full mt-0.5 px-2 py-1 text-xs bg-ink-800 border border-ink-700 rounded text-ink-200 focus:outline-none focus:border-ink-500"
      />
    </label>
  );
}

/** Shows model preset buttons when url matches a known provider, else a free-text input. */
function ModelSuggestions({ url, model, onSelect }: { url: string; model: string; onSelect: (m: string) => void }) {
  const presets = MODEL_PRESETS[url];
  if (presets) {
    return (
      <label className="block">
        <span className="text-[10px] text-ink-500">模型</span>
        <div className="flex gap-1 flex-wrap mt-0.5">
          {presets.map((m) => (
            <button
              key={m}
              onClick={() => onSelect(m)}
              className={`text-[10px] px-2 py-0.5 rounded-full border transition-colors ${
                model === m
                  ? "bg-ink-600 text-ink-100 border-ink-500"
                  : "bg-ink-800 text-ink-400 border-ink-700 hover:border-ink-500"
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

/** Numeric input for context window config values. */
function ContextInput({ label, value, onChange }: { label: string; value: number; onChange: (value: number) => void }) {
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
