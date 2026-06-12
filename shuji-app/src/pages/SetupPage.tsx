import { useState, useEffect } from 'react';
import { useNavigate } from 'react-router-dom';
import { getConfig, saveConfig, checkApiConnection, setModelPreset } from '../api';
import { formatError } from '../utils/error';
import { API_URL_PRESETS, MODEL_PRESETS } from '../constants/presets';
import type { AppConfig, RoleEndpoint } from '../types';
import { SealLogo } from '../components/SealLogo';
import { Card } from '../components/ui/Card';
import { Button } from '../components/ui/Button';

type Step = 1 | 2 | 3;

interface PresetOption {
  key: string;
  icon: string;
  label: string;
  description: string;
  detail: string;
}

const PRESETS: PresetOption[] = [
  {
    key: 'economy',
    icon: '⚡',
    label: '极速',
    description: '轻量模型，最快响应',
    detail: '适合学习和体验，设计/编码用轻量模型',
  },
  {
    key: 'balanced',
    icon: '⚖',
    label: '均衡',
    description: '性价比最优',
    detail: '设计用中号、编码用强号 （推荐）',
  },
  {
    key: 'quality',
    icon: '🎯',
    label: '极致',
    description: '最强模型最高质量',
    detail: '审查/检查用轻量、其余全用强号',
  },
];

export default function SetupPage() {
  const navigate = useNavigate();
  const [step, setStep] = useState<Step>(1);
  const [preset, setPreset] = useState('balanced');
  const [apiKey, setApiKey] = useState('');
  const [apiUrl, setApiUrl] = useState<string>(API_URL_PRESETS[0].url);
  const [customUrl, setCustomUrl] = useState('');
  const [model, setModel] = useState(MODEL_PRESETS[API_URL_PRESETS[0].url]?.[0] || '');
  const [showAdvanced, setShowAdvanced] = useState(false);
  const [saving, setSaving] = useState(false);
  const [testing, setTesting] = useState(false);
  const [testResult, setTestResult] = useState<'idle' | 'ok' | 'fail'>('idle');
  const [error, setError] = useState('');

  useEffect(() => {
    getConfig()
      .then((cfg) => {
        const def = cfg.roles?.default;
        if (def?.api_key) navigate('/', { replace: true });
      })
      .catch((e) => {
        console.error('加载配置失败', e);
      });
  }, []);

  const effectiveUrl = apiUrl || customUrl;

  const handleUrlPreset = (url: string) => {
    setApiUrl(url);
    if (url && MODEL_PRESETS[url]) setModel(MODEL_PRESETS[url][0]);
  };

  const handleTestConnection = async () => {
    if (!apiKey.trim() || !effectiveUrl || !model) return;
    setTesting(true);
    setTestResult('idle');
    try {
      await checkApiConnection(apiKey.trim(), effectiveUrl, model);
      setTestResult('ok');
    } catch {
      setTestResult('fail');
    } finally {
      setTesting(false);
    }
  };

  const handleSave = async () => {
    if (!apiKey.trim()) {
      setError('请输入 API 密钥');
      return;
    }
    setSaving(true);
    setError('');
    try {
      const roles: Record<string, RoleEndpoint> = {
        default: {
          api_key: apiKey.trim(),
          api_url: effectiveUrl || API_URL_PRESETS[0].url,
          model: model || MODEL_PRESETS[API_URL_PRESETS[0].url]?.[0] || '',
        },
      };
      const config: AppConfig = { preset, roles };
      await saveConfig(config);
      await setModelPreset(preset).catch((e) => console.error('设置模型预设失败', e));
      setStep(3);
    } catch (e) {
      setError(formatError(e));
    } finally {
      setSaving(false);
    }
  };

  return (
    <div className="h-screen bg-surface-paper flex items-center justify-center">
      <div className="w-full max-w-lg mx-4">
        {/* Header */}
        <div className="text-center mb-6">
          <div className="flex justify-center mb-3">
            <SealLogo size={40} />
          </div>
          <h1 className="font-display text-display font-bold text-ink-900 tracking-wide mb-2">
            枢机
          </h1>
          <p className="text-body text-ink-600">三省六部制自动化软件开发系统</p>
        </div>

        {/* Step indicator */}
        <div className="flex items-center justify-center gap-2 mb-6">
          {([1, 2, 3] as const).map((s) => (
            <div key={s} className="flex items-center gap-2">
              <div
                className={`w-7 h-7 rounded-full flex items-center justify-center text-ui font-bold transition-colors ${
                  step === s
                    ? 'bg-vermillion text-white'
                    : step > s
                      ? 'bg-jade text-white'
                      : 'bg-ink-200 text-ink-500'
                }`}
              >
                {step > s ? '✓' : s}
              </div>
              <span
                className={`text-ui ${step === s ? 'text-ink-900 font-medium' : 'text-ink-400'}`}
              >
                {s === 1 ? '预设' : s === 2 ? '密钥' : '完成'}
              </span>
              {s < 3 && <div className="w-6 h-px bg-ink-300 mx-1" />}
            </div>
          ))}
        </div>

        {step === 1 && (
          <Card variant="paper" className="p-6 space-y-4">
            <h2 className="font-display text-sm font-bold text-ink-900 text-center">
              选择你的使用偏好
            </h2>
            <div className="space-y-2">
              {PRESETS.map((p) => (
                <button
                  key={p.key}
                  onClick={() => setPreset(p.key)}
                  className={`w-full text-left p-3 rounded-lg border transition-colors ${
                    preset === p.key
                      ? 'bg-vermillion-light border-vermillion text-ink-900'
                      : 'bg-surface-parchment border-fold text-ink-600 hover:border-ink-400'
                  }`}
                >
                  <div className="flex items-center gap-2">
                    <span className="text-lg">{p.icon}</span>
                    <div>
                      <div className="text-ui font-bold">
                        {p.label}
                        {p.key === 'balanced' && (
                          <span className="ml-1 text-caption text-vermillion font-normal">
                            ★ 推荐
                          </span>
                        )}
                      </div>
                      <div className="text-caption opacity-70">{p.description}</div>
                    </div>
                  </div>
                  <div className="text-caption text-ink-500 mt-1 ml-7">{p.detail}</div>
                </button>
              ))}
            </div>
            <div className="flex gap-2 pt-2">
              <Button variant="ghost" className="flex-1" onClick={() => navigate('/setup?skip=1')}>
                跳过上手指南
              </Button>
              <Button variant="primary" className="flex-1" onClick={() => setStep(2)}>
                下一步
              </Button>
            </div>
          </Card>
        )}

        {step === 2 && (
          <Card variant="paper" className="p-6 space-y-4">
            <h2 className="font-display text-sm font-bold text-ink-900 text-center">
              配置 API 密钥
            </h2>

            {/* API Key */}
            <div>
              <label className="block text-ui font-medium text-ink-600 mb-1.5">
                API 密钥 <span className="text-vermillion">*</span>
              </label>
              <input
                type="password"
                value={apiKey}
                onChange={(e) => {
                  setApiKey(e.target.value);
                  setError('');
                }}
                placeholder="sk-..."
                autoFocus
                className="w-full px-3 py-2 text-body border border-fold rounded-lg bg-surface-parchment text-ink-900 placeholder-ink-400 focus:outline-none focus:ring-2 focus:ring-vermillion/30 focus:border-vermillion transition-colors"
              />
              <p className="text-caption text-ink-400 mt-1">密钥仅保存在本地，不会上传</p>
            </div>

            {/* API URL */}
            <div>
              <label className="block text-ui font-medium text-ink-600 mb-1.5">服务商</label>
              <div className="flex gap-1.5 flex-wrap mb-2">
                {API_URL_PRESETS.map((p) => (
                  <button
                    key={p.label}
                    onClick={() => handleUrlPreset(p.url)}
                    className={`text-ui px-2.5 py-1 rounded-full border transition-colors ${
                      (p.url && apiUrl === p.url) || (!p.url && !apiUrl)
                        ? 'bg-vermillion text-white border-vermillion'
                        : 'bg-surface-parchment text-ink-500 border-fold hover:border-ink-400'
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
                  className="w-full px-3 py-2 text-body border border-fold rounded-lg bg-surface-parchment text-ink-900 placeholder-ink-400 focus:outline-none focus:ring-2 focus:ring-vermillion/30 focus:border-vermillion"
                />
              )}
            </div>

            {/* Model */}
            <div>
              <label className="block text-ui font-medium text-ink-600 mb-1.5">模型</label>
              {MODEL_PRESETS[effectiveUrl] ? (
                <div className="flex gap-1.5 flex-wrap">
                  {MODEL_PRESETS[effectiveUrl].map((m) => (
                    <button
                      key={m}
                      onClick={() => setModel(m)}
                      className={`text-ui px-2.5 py-1 rounded-full border transition-colors ${
                        model === m
                          ? 'bg-ink-900 text-white border-ink-900'
                          : 'bg-surface-parchment text-ink-500 border-fold hover:border-ink-400'
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
                  className="w-full px-3 py-2 text-body border border-fold rounded-lg bg-surface-parchment text-ink-900 placeholder-ink-400 focus:outline-none focus:ring-2 focus:ring-vermillion/30 focus:border-vermillion"
                />
              )}
            </div>

            {/* Test connection */}
            <div className="flex items-center gap-2">
              <Button
                variant="secondary"
                onClick={handleTestConnection}
                disabled={testing || !apiKey.trim()}
              >
                {testing ? '测试中...' : '测试连接'}
              </Button>
              {testResult === 'ok' && <span className="text-ui text-jade">✔ 连接成功</span>}
              {testResult === 'fail' && (
                <span className="text-ui text-vermillion">✘ 连接失败，请检查配置</span>
              )}
            </div>

            {/* Advanced */}
            <div>
              <button
                onClick={() => setShowAdvanced(!showAdvanced)}
                className="text-ui text-ink-400 hover:text-ink-600 transition-colors"
              >
                {showAdvanced ? '▾ 高级配置' : '▸ 高级配置'}
              </button>
              {showAdvanced && (
                <div className="mt-2 p-3 bg-surface-parchment rounded-lg text-caption text-ink-500 space-y-1">
                  <p>
                    进入主界面后点击右上角 <strong>设置</strong>，可为各部门分别配置 API。
                  </p>
                  <p>当前默认 key 将被所有部门共享，除非在设置中单独覆盖。</p>
                  <p>模型分级预设只影响角色 model 字段，不改 API URL/Key。</p>
                </div>
              )}
            </div>

            {error && (
              <div className="bg-vermillion-light border border-vermillion/20 text-vermillion-dark px-3 py-2 rounded text-ui">
                {error}
              </div>
            )}

            <div className="flex gap-3 pt-1">
              <Button variant="ghost" className="flex-1" onClick={() => setStep(1)}>
                上一步
              </Button>
              <Button variant="primary" className="flex-1" disabled={saving} onClick={handleSave}>
                {saving ? '保存中...' : '保存并继续'}
              </Button>
            </div>
          </Card>
        )}

        {step === 3 && (
          <Card variant="paper" className="p-6 space-y-5 text-center">
            <div className="w-12 h-12 mx-auto rounded-full bg-jade-light flex items-center justify-center">
              <svg
                className="w-6 h-6 text-jade"
                fill="none"
                viewBox="0 0 24 24"
                stroke="currentColor"
                strokeWidth={2.5}
              >
                <path strokeLinecap="round" strokeLinejoin="round" d="M4.5 12.75l6 6 9-13.5" />
              </svg>
            </div>
            <div>
              <h2 className="font-display text-sm font-bold text-ink-900">配置已保存！</h2>
              <p className="text-body text-ink-600 mt-1">现在可以开始使用枢机了</p>
            </div>
            <div className="space-y-2">
              <Button
                variant="primary"
                className="w-full"
                onClick={() => navigate('/', { replace: true })}
              >
                🚀 开始第一个项目
              </Button>
              <Button
                variant="secondary"
                className="w-full"
                onClick={() => {
                  navigate('/');
                }}
              >
                ⚡ 先跑一个 Demo
              </Button>
              <Button
                variant="ghost"
                className="w-full"
                onClick={() => navigate('/', { replace: true })}
              >
                返回首页
              </Button>
            </div>
          </Card>
        )}
      </div>
    </div>
  );
}
