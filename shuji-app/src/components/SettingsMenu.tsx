import { useState } from "react";
import { getConfig, saveConfig, getContextConfig, saveContextConfig } from "../api";
import { ALL_ROLES } from "../constants";
import type { RoleEndpoint, ContextWindowConfig } from "../types";

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
  char_threshold: number;
  keep_recent_count: number;
  history_char_threshold: number;
}

/// Matches default values in config/mod.rs
const DEFAULT_CONTEXT_VALUES: ContextRoleForm = {
  char_threshold: 80_000,
  keep_recent_count: 10,
  history_char_threshold: 2_000,
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
        if (roles[role.key]) {
          overrides[role.key] = roles[role.key] as ContextRoleForm;
          useDefault[role.key] = false;
        } else {
          useDefault[role.key] = true;
        }
      }
      setContextOverrides(overrides);
      setContextUseDefault(useDefault);
    }).catch((e) => console.error("读取上下文配置失败:", e));
  };

  const toggle = () => {
    if (!open) { loadConfig(); loadContextConfig(); }
    setOpen(!open);
  };

  const setOverride = (role: string, field: keyof RoleFormState, value: string) => {
    setOverrides((prev) => ({ ...prev, [role]: { ...(prev[role] ?? defaultCfg), [field]: value } }));
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

  const toggleContextDefault = (role: string) => {
    setContextUseDefault((prev) => {
      const current = prev[role] ?? true;
      if (current) {
        setContextOverrides((o) => ({ ...o, [role]: { ...DEFAULT_CONTEXT_VALUES } }));
      }
      return { ...prev, [role]: !current };
    });
  };

  const setContextOverride = (role: string, field: keyof ContextRoleForm, value: number) => {
    setContextOverrides((prev) => ({
      ...prev,
      [role]: { ...(prev[role] ?? DEFAULT_CONTEXT_VALUES), [field]: value },
    }));
  };

  const effectiveContext = (key: string): ContextRoleForm =>
    !(contextUseDefault[key] ?? true) && contextOverrides[key] ? contextOverrides[key] : DEFAULT_CONTEXT_VALUES;

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

      // Save context window config
      const ctxRoles: Record<string, ContextRoleForm> = {};
      for (const role of ALL_ROLES) {
        if (role.key === "default") continue;
        if (!(contextUseDefault[role.key] ?? true)) {
          ctxRoles[role.key] = contextOverrides[role.key] ?? DEFAULT_CONTEXT_VALUES;
        }
      }
      await saveContextConfig({ roles: ctxRoles });

      setSavedMsg("已保存");
      setTimeout(() => setSavedMsg(""), 2000);
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
              全局默认: {DEFAULT_CONTEXT_VALUES.char_threshold.toLocaleString()} token / 保留{DEFAULT_CONTEXT_VALUES.keep_recent_count}条 / {DEFAULT_CONTEXT_VALUES.history_char_threshold.toLocaleString()} token 摘要
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
                      {effectiveContext(r.key).char_threshold.toLocaleString()} token
                    </span>
                  </button>
                  {isExpanded && (
                    <div className="px-2 pb-2 space-y-1">
                      {usingDefault ? (
                        <div className="text-[10px] text-ink-500 italic px-1 py-2">
                          使用全局默认值
                          <br />
                          取消勾选"使用默认"可单独设置
                        </div>
                      ) : (
                        <>
                          <ContextInput label="压缩阈值（token）" value={contextOverrides[r.key]?.char_threshold ?? DEFAULT_CONTEXT_VALUES.char_threshold} onChange={(v) => setContextOverride(r.key, "char_threshold", v)} />
                          <ContextInput label="保留最近消息数" value={contextOverrides[r.key]?.keep_recent_count ?? DEFAULT_CONTEXT_VALUES.keep_recent_count} onChange={(v) => setContextOverride(r.key, "keep_recent_count", v)} />
                          <ContextInput label="历史摘要合并阈值（token）" value={contextOverrides[r.key]?.history_char_threshold ?? DEFAULT_CONTEXT_VALUES.history_char_threshold} onChange={(v) => setContextOverride(r.key, "history_char_threshold", v)} />
                        </>
                      )}
                    </div>
                  )}
                </div>
              );
            })}
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
