import { useState, useEffect } from "react";
import { useNavigate } from "react-router-dom";
import { getConfig, saveConfig } from "../api";
import type { AppConfig } from "../types";

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

export default function SetupPage() {
  const navigate = useNavigate();
  const [apiKey, setApiKey] = useState("");
  const [apiUrl, setApiUrl] = useState(API_URL_PRESETS[0].url);
  const [customUrl, setCustomUrl] = useState("");
  const [model, setModel] = useState(MODEL_PRESETS[API_URL_PRESETS[0].url]?.[0] || "");
  const [showAdvanced, setShowAdvanced] = useState(false);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState("");

  useEffect(() => {
    getConfig()
      .then((cfg) => {
        const def = cfg.roles?.default;
        if (def?.api_key) {
          navigate("/", { replace: true });
        }
      })
      .catch((e) => console.error("读取配置失败:", e));
  }, []);

  const effectiveUrl = apiUrl || customUrl;

  const handleUrlPreset = (url: string) => {
    setApiUrl(url);
    if (url && MODEL_PRESETS[url]) {
      setModel(MODEL_PRESETS[url][0]);
    }
  };

  const handleSave = async () => {
    if (!apiKey.trim()) {
      setError("请输入 API 密钥");
      return;
    }
    setSaving(true);
    setError("");
    try {
      const config: AppConfig = {
        roles: {
          default: {
            api_key: apiKey.trim(),
            api_url: effectiveUrl || API_URL_PRESETS[0].url,
            model: model || MODEL_PRESETS[API_URL_PRESETS[0].url]?.[0] || "",
          },
        },
      };
      await saveConfig(config);
      navigate("/", { replace: true });
    } catch (e) {
      setError(String(e));
    } finally {
      setSaving(false);
    }
  };

  const handleSkip = () => {
    navigate("/", { replace: true });
  };

  return (
    <div className="h-screen bg-ink-50 flex items-center justify-center">
      <div className="w-full max-w-md mx-4">
        {/* Header */}
        <div className="text-center mb-8">
          <h1 className="text-2xl font-bold text-ink-900 tracking-wide mb-2">枢机</h1>
          <p className="text-sm text-ink-500">欢迎使用。请先配置 API 密钥以开始使用。</p>
        </div>

        {/* Form */}
        <div className="bg-white border border-ink-200 rounded-xl p-6 space-y-5 shadow-sm">
          {/* API Key */}
          <div>
            <label className="block text-xs font-medium text-ink-600 mb-1.5">
              API 密钥 <span className="text-vermillion">*</span>
            </label>
            <input
              type="password"
              value={apiKey}
              onChange={(e) => { setApiKey(e.target.value); setError(""); }}
              placeholder="sk-..."
              className="w-full px-3 py-2 text-sm border border-ink-200 rounded-lg bg-ink-50 text-ink-900 placeholder-ink-400 focus:outline-none focus:ring-2 focus:ring-vermillion/30 focus:border-vermillion transition-colors"
            />
            <p className="text-[11px] text-ink-400 mt-1">
              密钥仅保存在本地，不会上传
            </p>
          </div>

          {/* API URL */}
          <div>
            <label className="block text-xs font-medium text-ink-600 mb-1.5">服务商</label>
            <div className="flex gap-1.5 flex-wrap mb-2">
              {API_URL_PRESETS.map((p) => (
                <button
                  key={p.label}
                  onClick={() => handleUrlPreset(p.url)}
                  className={`text-xs px-2.5 py-1 rounded-full border transition-colors ${
                    (p.url && apiUrl === p.url) || (!p.url && !apiUrl)
                      ? "bg-vermillion text-white border-vermillion"
                      : "bg-ink-50 text-ink-500 border-ink-200 hover:border-ink-400"
                  }`}
                >
                  {p.label}
                </button>
              ))}
            </div>
            {!apiUrl && (
              <input
                type="text"
                value={customUrl}
                onChange={(e) => setCustomUrl(e.target.value)}
                placeholder="https://your-api.com/chat/completions"
                className="w-full px-3 py-2 text-sm border border-ink-200 rounded-lg bg-ink-50 text-ink-900 placeholder-ink-400 focus:outline-none focus:ring-2 focus:ring-vermillion/30 focus:border-vermillion"
              />
            )}
          </div>

          {/* Model */}
          <div>
            <label className="block text-xs font-medium text-ink-600 mb-1.5">模型</label>
            {MODEL_PRESETS[effectiveUrl] ? (
              <div className="flex gap-1.5 flex-wrap">
                {MODEL_PRESETS[effectiveUrl].map((m) => (
                  <button
                    key={m}
                    onClick={() => setModel(m)}
                    className={`text-xs px-2.5 py-1 rounded-full border transition-colors ${
                      model === m
                        ? "bg-ink-900 text-white border-ink-900"
                        : "bg-ink-50 text-ink-500 border-ink-200 hover:border-ink-400"
                    }`}
                  >
                    {m}
                  </button>
                ))}
              </div>
            ) : (
              <input
                type="text"
                value={model}
                onChange={(e) => setModel(e.target.value)}
                placeholder="model-name"
                className="w-full px-3 py-2 text-sm border border-ink-200 rounded-lg bg-ink-50 text-ink-900 placeholder-ink-400 focus:outline-none focus:ring-2 focus:ring-vermillion/30 focus:border-vermillion"
              />
            )}
          </div>

          {/* Advanced */}
          <div>
            <button
              onClick={() => setShowAdvanced(!showAdvanced)}
              className="text-xs text-ink-400 hover:text-ink-600 transition-colors"
            >
              {showAdvanced ? "▾ 高级配置" : "▸ 高级配置"}
            </button>
            {showAdvanced && (
              <div className="mt-2 p-3 bg-ink-50 rounded-lg text-[11px] text-ink-500 space-y-1">
                <p>进入主界面后点击右上角 <strong>设置</strong>，可为各部门分别配置 API。</p>
                <p>当前默认 key 将被所有部门共享，除非在设置中单独覆盖。</p>
              </div>
            )}
          </div>

          {/* Error */}
          {error && (
            <div className="bg-vermillion-light border border-vermillion/20 text-vermillion-dark px-3 py-2 rounded text-xs">
              {error}
            </div>
          )}

          {/* Actions */}
          <div className="flex gap-3 pt-1">
            <button
              onClick={handleSkip}
              className="flex-1 py-2 text-sm text-ink-400 hover:text-ink-600 border border-ink-200 rounded-lg hover:bg-ink-50 transition-colors"
            >
              跳过
            </button>
            <button
              onClick={handleSave}
              disabled={saving}
              className="flex-1 py-2 text-sm font-medium bg-ink-900 text-white rounded-lg hover:bg-ink-800 disabled:opacity-50 transition-colors"
            >
              {saving ? "保存中..." : "开始使用"}
            </button>
          </div>
        </div>

        <p className="text-center text-[11px] text-ink-400 mt-6">
          密钥仅保存在本地，可在进入主界面后随时修改
        </p>
      </div>
    </div>
  );
}
