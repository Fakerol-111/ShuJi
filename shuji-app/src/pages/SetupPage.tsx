import { useState, useEffect } from 'react';
import { useNavigate } from 'react-router-dom';
import { useTranslation } from 'react-i18next';
import { getConfig, saveConfig, checkApiConnection, setModelPreset } from '../api';
import { formatError } from '../utils/error';
import { API_URL_PRESETS, MODEL_PRESETS, detectProvider } from '../constants/presets';
import { DEPT_META_BY_KEY } from '../constants';
import type { AppConfig, RoleEndpoint } from '../types';
import { SealLogo } from '../components/SealLogo';
import { Card } from '../components/ui/Card';
import { Button } from '../components/ui/Button';

type Step = 1 | 2 | 3 | 4;

interface PresetOption {
  key: string;
  icon: string;
  label: string;
  description: string;
  detail: string;
}

/** Derive per-role models from the default model + preset. Mirrors backend apply_model_preset. */
function deriveRoleModels(
  apiUrl: string,
  defaultModel: string,
  preset: string
): { key: string; label: string; model: string }[] {
  const cheapRoles = ['menxiashizhong', 'xingbushangshu', 'liburshangshu'];
  const strongRoles = ['zhongshuling', 'gongbushangshu', 'libushangshu'];
  const depts = [
    { key: 'neige', label: '内阁' },
    { key: 'zhongshuling', label: '中书令' },
    { key: 'menxiashizhong', label: '门下侍中' },
    { key: 'shangshuling', label: '尚书令' },
    { key: 'libushangshu', label: '吏部尚书' },
    { key: 'bingbushangshu', label: '兵部尚书' },
    { key: 'gongbushangshu', label: '工部尚书' },
    { key: 'xingbushangshu', label: '刑部尚书' },
    { key: 'liburshangshu', label: '礼部尚书' },
  ];

  if (preset === 'economy') {
    const isDeepSeek = apiUrl.includes('deepseek.com');
    const isAnthropic = apiUrl.includes('anthropic.com');
    const cheapModel = isDeepSeek
      ? 'deepseek-v4-flash'
      : isAnthropic
        ? 'claude-haiku-4-5-20251001'
        : 'gpt-4o-mini';
    const strongModel = isDeepSeek
      ? 'deepseek-4-pro'
      : isAnthropic
        ? 'claude-sonnet-4-20250514'
        : 'gpt-4o';
    return depts.map((d) => ({
      ...d,
      model: cheapRoles.includes(d.key)
        ? cheapModel
        : strongRoles.includes(d.key)
          ? strongModel
          : defaultModel,
    }));
  }

  if (preset === 'quality') {
    const isDeepSeek = apiUrl.includes('deepseek.com');
    const isAnthropic = apiUrl.includes('anthropic.com');
    const cheapModel = isDeepSeek
      ? 'deepseek-v4-flash'
      : isAnthropic
        ? 'claude-haiku-4-5-20251001'
        : 'gpt-4o-mini';
    const strongModel = isDeepSeek
      ? 'deepseek-4-pro'
      : isAnthropic
        ? 'claude-sonnet-4-20250514'
        : 'gpt-4o';
    return depts.map((d) => ({
      ...d,
      model: strongRoles.includes(d.key)
        ? strongModel
        : cheapRoles.includes(d.key)
          ? cheapModel
          : defaultModel,
    }));
  }

  // balanced or custom: all roles use the default model
  return depts.map((d) => ({ ...d, model: defaultModel }));
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
  const { t } = useTranslation();
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
      setError(t('setup.apiKeyRequired'));
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

  const roleModels: { key: string; label: string; model: string }[] = (() => {
    const derived = deriveRoleModels(effectiveUrl || API_URL_PRESETS[0].url, model, preset);
    return [{ key: 'default', label: '默认（全局）', model: model || '' }, ...derived];
  })();

  return (
    <div className="h-screen bg-surface-paper flex items-center justify-center">
      <div className="w-full max-w-lg mx-4">
        {/* Header */}
        <div className="text-center mb-6">
          <div className="flex justify-center mb-3">
            <SealLogo size={40} />
          </div>
          <h1 className="font-display text-display font-bold text-ink-900 tracking-wide mb-2">
            {t('app.name')}
          </h1>
          <p className="text-body text-ink-600">{t('app.subtitle')}</p>
        </div>

        {/* Step indicator */}
        <div className="flex items-center justify-center gap-2 mb-6">
          {([1, 2, 3, 4] as const).map((s) => (
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
                {s === 1 ? t('setup.preset') : s === 2 ? t('setup.apiKey') : s === 3 ? t('setup.role') : t('common.finish')}
              </span>
              {s < 4 && <div className="w-6 h-px bg-ink-300 mx-1" />}
            </div>
          ))}
        </div>

        {step === 1 && (
          <Card variant="paper" className="p-6 space-y-4">
            <h2 className="font-display text-sm font-bold text-ink-900 text-center">
              {t('setup.choosePreference')}
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
                            {t('setup.recommended')}
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
                {t('setup.skipTutorial')}
              </Button>
              <Button variant="primary" className="flex-1" onClick={() => setStep(2)}>
                {t('setup.next')}
              </Button>
            </div>
          </Card>
        )}

        {step === 2 && (
          <Card variant="paper" className="p-6 space-y-4">
            <h2 className="font-display text-sm font-bold text-ink-900 text-center">
              {t('setup.configureApi')}
            </h2>

            {/* API Key */}
            <div>
              <label className="block text-ui font-medium text-ink-600 mb-1.5">
                {t('setup.apiKey')} <span className="text-vermillion">*</span>
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
              <p className="text-caption text-ink-400 mt-1">{t('setup.keysStoredLocally')}</p>
            </div>

            {/* API URL */}
            <div>
              <label className="block text-ui font-medium text-ink-600 mb-1.5">{t('setup.provider')}</label>
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
                {testing ? t('setup.testing') : t('setup.testConnection')}
              </Button>
              {testResult === 'ok' && <span className="text-ui text-jade">{t('setup.connectionSuccess')}</span>}
              {testResult === 'fail' && (
                <span className="text-ui text-vermillion">{t('setup.connectionFailHint')}</span>
              )}
            </div>

            {/* Advanced */}
            <div>
              <button
                onClick={() => setShowAdvanced(!showAdvanced)}
                className="text-ui text-ink-400 hover:text-ink-600 transition-colors"
              >
                {showAdvanced ? '▾ ' + t('setup.advancedConfig') : '▸ ' + t('setup.advancedConfig')}
              </button>
              {showAdvanced && (
                <div className="mt-2 p-3 bg-surface-parchment rounded-lg text-caption text-ink-500 space-y-1">
                  <p>{t('setup.advancedHint1')}</p>
                  <p>{t('setup.advancedHint2')}</p>
                  <p>{t('setup.advancedHint3')}</p>
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
                {t('setup.back')}
              </Button>
              <Button variant="primary" className="flex-1" disabled={saving} onClick={handleSave}>
                {saving ? t('setup.saving') : t('setup.saveAndContinue')}
              </Button>
            </div>
          </Card>
        )}

        {step === 3 && (
          <Card variant="paper" className="p-6 space-y-4">
            <h2 className="font-display text-sm font-bold text-ink-900 text-center">
              {t('setup.roleOverviewTitle')}
            </h2>
            <p className="text-caption text-ink-500 text-center">
              {t('setup.roleOverviewDesc')}
            </p>
            <div className="space-y-1 max-h-[320px] overflow-y-auto">
              {roleModels.map((r) => {
                const meta = r.key !== 'default' ? DEPT_META_BY_KEY[r.key] : undefined;
                const providerLabel = detectProvider(effectiveUrl || API_URL_PRESETS[0].url);
                return (
                  <div
                    key={r.key}
                    className="flex items-center gap-3 px-3 py-2 rounded-lg bg-surface-parchment border border-fold"
                  >
                    <div
                      className="w-2 h-2 rounded-full shrink-0"
                      style={{ backgroundColor: meta?.color || '#8B7355' }}
                    />
                    <span className="text-ui font-medium text-ink-700 w-16 shrink-0">
                      {meta?.shortLabel || r.label}
                    </span>
                    <span className="text-caption text-ink-500 truncate flex-1">{r.model}</span>
                    <span className="text-[10px] text-ink-400 shrink-0">{providerLabel}</span>
                  </div>
                );
              })}
            </div>
            <div className="flex gap-2 pt-1">
              <Button variant="ghost" className="flex-1" onClick={() => setStep(2)}>
                {t('setup.back')}
              </Button>
              <Button variant="primary" className="flex-1" onClick={() => setStep(4)}>
                {t('setup.confirmAndFinish')}
              </Button>
            </div>
          </Card>
        )}

        {step === 4 && (
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
              <h2 className="font-display text-sm font-bold text-ink-900">{t('setup.configSaved')}</h2>
              <p className="text-body text-ink-600 mt-1">{t('setup.configSavedDesc')}</p>
            </div>
            <div className="space-y-2">
              <Button
                variant="primary"
                className="w-full"
                onClick={() => navigate('/', { replace: true })}
              >
                {'🚀 ' + t('setup.startFirstProject')}
              </Button>
              <Button
                variant="secondary"
                className="w-full"
                onClick={() => {
                  navigate('/');
                }}
              >
                {'⚡ ' + t('setup.runDemo')}
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
