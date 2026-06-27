import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import type { PipelineRuntime } from '../types';

type PlanStatus = 'pending' | 'in_progress' | 'done' | 'failed' | 'skipped';

const STATUS_ICONS: Record<PlanStatus, string> = {
  pending: '⬜',
  in_progress: '🔄',
  done: '✅',
  failed: '❌',
  skipped: '⏭️',
};

export interface PlanPanelProps {
  runtime: PipelineRuntime | null;
  defaultExpanded?: boolean;
  className?: string;
}

export default function PlanPanel({
  runtime,
  defaultExpanded = true,
  className = '',
}: PlanPanelProps) {
  const { t } = useTranslation();
  const [expanded, setExpanded] = useState(defaultExpanded);

  if (!runtime) return null;

  const totalSteps = runtime.plan.steps.length;
  const doneSteps = runtime.plan.steps.filter((s) => {
    const st = runtime.step_status[s.step_id];
    return st === 'done' || st === 'skipped';
  }).length;
  const pct = totalSteps > 0 ? Math.round((doneSteps / totalSteps) * 100) : 0;

  return (
    <div className={`rounded-lg border border-fold/60 bg-surface-paper/80 ${className}`}>
      <button
        type="button"
        className="w-full flex items-center gap-2 px-2 py-1.5 text-left hover:bg-ink-100/30 rounded-t-lg"
        onClick={() => setExpanded((v) => !v)}
      >
        <span className="text-ink-500 font-medium shrink-0">{t('plan.pipelinePlan')}</span>
        <span className="text-ink-700 truncate flex-1 min-w-0">{runtime.plan.summary}</span>
        <span className="text-caption font-mono text-ink-500 shrink-0">
          {doneSteps}/{totalSteps}
        </span>
        <span className="text-caption text-ink-400 shrink-0">{expanded ? '▾' : '▸'}</span>
      </button>

      <div className="h-1 mx-2 mb-1 bg-ink-200 rounded-full overflow-hidden">
        <div className="h-full bg-gold transition-all duration-500" style={{ width: `${pct}%` }} />
      </div>

      {expanded && (
        <div className="px-2 pb-2 space-y-0.5 max-h-40 overflow-y-auto">
          {runtime.plan.steps.map((step) => {
            const status = (runtime.step_status[step.step_id] || 'pending') as PlanStatus;
            const isCurrent = runtime.current_step === step.step_id;

            return (
              <div
                key={step.step_id}
                className={`flex items-center gap-1.5 text-caption py-0.5 px-1 rounded ${
                  isCurrent ? 'bg-gold/10 text-ink-900' : 'text-ink-600'
                } ${status === 'failed' ? 'text-vermillion' : ''}`}
              >
                <span>{STATUS_ICONS[status] || '⬜'}</span>
                <span className="font-mono text-ink-400 shrink-0">{step.step_id}</span>
                <span className="truncate flex-1">{step.description}</span>
                {step.action === 'approval_gate' && (
                  <span className="shrink-0 px-1 rounded border border-vermillion/30 text-vermillion text-[10px]">
                    {t('plan.approval')}
                  </span>
                )}
                {step.action === 'ask_user' && (
                  <span className="shrink-0 px-1 rounded border border-gold/30 text-gold-700 text-[10px]">
                    {t('plan.question')}
                  </span>
                )}
              </div>
            );
          })}
        </div>
      )}

      {runtime.error_log.length > 0 && (
        <div className="px-2 pb-2 space-y-0.5 border-t border-fold/40 pt-1">
          {runtime.error_log.map((err, i) => (
            <div key={i} className="text-caption text-vermillion truncate">
              {err}
            </div>
          ))}
        </div>
      )}
    </div>
  );
}
