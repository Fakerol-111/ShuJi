import { useTranslation } from 'react-i18next';
import type { WorkflowConfig as WFConfig } from '../../types';

interface WorkflowSettingsTabProps {
  workflowIntent: string;
  setWorkflowIntent: (key: string) => void;
  workflowPreset: string;
  setWorkflowPresetLocal: (key: string) => void;
  modelPreset: string;
  setModelPresetLocal: (key: string) => void;
}

export default function WorkflowSettingsTab({
  workflowIntent,
  setWorkflowIntent,
  workflowPreset,
  setWorkflowPresetLocal,
  modelPreset,
  setModelPresetLocal,
}: WorkflowSettingsTabProps) {
  const { t } = useTranslation();

  const INTENTS: { key: string; labelKey: string; descKey: string }[] = [
    { key: 'auto', labelKey: 'workflow.intentAuto', descKey: 'settings.intentAutoDesc' },
    {
      key: 'greenfield_standard',
      labelKey: 'workflow.newFeature',
      descKey: 'settings.intentNewFeatureDesc',
    },
    {
      key: 'brownfield_optimize',
      labelKey: 'workflow.existingOptimization',
      descKey: 'settings.intentOptimizeDesc',
    },
    { key: 'bugfix', labelKey: 'workflow.bugfix', descKey: 'settings.intentBugfixDesc' },
    { key: 'demo', labelKey: 'workflow.quickPrototype', descKey: 'settings.intentDemoDesc' },
  ];

  const PRESETS: { key: WFConfig['governance']; labelKey: string; descKey: string }[] = [
    { key: 'full', labelKey: 'workflow.governanceFull', descKey: 'settings.presetFullDesc' },
    {
      key: 'standard',
      labelKey: 'workflow.governanceStandard',
      descKey: 'settings.presetStandardDesc',
    },
    { key: 'fast', labelKey: 'workflow.governanceFast', descKey: 'settings.presetFastDesc' },
    { key: 'audit', labelKey: 'workflow.governanceAudit', descKey: 'settings.presetAuditDesc' },
  ];

  const MODEL_PRESETS: { key: string; labelKey: string; descKey: string }[] = [
    {
      key: 'balanced',
      labelKey: 'setup.presetBalanced',
      descKey: 'settings.modelPresetBalancedDesc',
    },
    {
      key: 'economy',
      labelKey: 'setup.presetEconomyLabel',
      descKey: 'settings.modelPresetEconomyDesc',
    },
    { key: 'quality', labelKey: 'setup.presetQuality', descKey: 'settings.modelPresetQualityDesc' },
  ];

  return (
    <div className="space-y-3">
      {/* ── Workflow Intent ── */}
      <div className="space-y-1">
        <span className="text-[11px] font-semibold text-ink-300">
          {t('settings.workflowIntent')}
        </span>
        <div className="flex gap-1 flex-wrap">
          {INTENTS.map((p) => (
            <button
              key={p.key}
              onClick={() => setWorkflowIntent(p.key)}
              className={`text-[10px] px-2 py-1 rounded-full border transition-colors ${
                workflowIntent === p.key
                  ? 'bg-ink-700 text-ink-100 border-ink-600'
                  : 'bg-ink-800 text-ink-400 border-ink-700 hover:border-ink-500'
              }`}
              title={t(p.descKey)}
            >
              {t(p.labelKey)}
            </button>
          ))}
        </div>
        <div className="text-[10px] text-ink-500 px-1">
          {INTENTS.find((i) => i.key === workflowIntent)
            ? t(INTENTS.find((i) => i.key === workflowIntent)!.descKey)
            : ''}
        </div>
      </div>

      {/* ── Workflow preset ── */}
      <div className="space-y-1 pt-2 border-t border-ink-700">
        <span className="text-[11px] font-semibold text-ink-300">
          {t('settings.workflowPreset')}
        </span>
        <div className="flex gap-1 flex-wrap">
          {PRESETS.map((p) => (
            <button
              key={p.key}
              onClick={() => setWorkflowPresetLocal(p.key)}
              className={`text-[10px] px-2 py-1 rounded-full border transition-colors ${
                workflowPreset === p.key
                  ? 'bg-ink-700 text-ink-100 border-ink-600'
                  : 'bg-ink-800 text-ink-400 border-ink-700 hover:border-ink-500'
              }`}
              title={t(p.descKey)}
            >
              {t(p.labelKey)}
            </button>
          ))}
        </div>
        <div className="text-[10px] text-ink-500 px-1">
          {PRESETS.find((p) => p.key === workflowPreset)
            ? t(PRESETS.find((p) => p.key === workflowPreset)!.descKey)
            : ''}
        </div>
      </div>

      {/* ── Model preset ── */}
      <div className="space-y-1 pt-2 border-t border-ink-700">
        <span className="text-[11px] font-semibold text-ink-300">{t('settings.modelPreset')}</span>
        <div className="flex gap-1 flex-wrap items-center">
          {MODEL_PRESETS.map((p) => (
            <button
              key={p.key}
              onClick={() => setModelPresetLocal(p.key)}
              className={`text-[10px] px-2 py-1 rounded-full border transition-colors ${
                modelPreset === p.key
                  ? 'bg-ink-700 text-ink-100 border-ink-600'
                  : 'bg-ink-800 text-ink-400 border-ink-700 hover:border-ink-500'
              }`}
              title={t(p.descKey)}
            >
              {t(p.labelKey)}
            </button>
          ))}
          {modelPreset === 'custom' && (
            <span className="text-[10px] text-ink-400 italic">{t('common.custom')}</span>
          )}
        </div>
        <div className="text-[10px] text-ink-500 px-1">
          {MODEL_PRESETS.find((p) => p.key === modelPreset)
            ? t(MODEL_PRESETS.find((p) => p.key === modelPreset)!.descKey)
            : t('settings.modelPresetCustomDesc')}
        </div>
        <div className="text-[10px] text-ink-400 px-1">{t('settings.modelPresetHint')}</div>
      </div>
    </div>
  );
}
