import { useEffect, useState } from 'react';
import { listen } from '@tauri-apps/api/event';
import { invoke } from '@tauri-apps/api/core';

// ── Types matching Rust pipeline structures ─────────────────

interface PlanStep {
  step_id: string;
  description: string;
  action: string;
  depends_on: string[];
  require_approval: boolean;
}

interface PipelinePlan {
  plan_id: string;
  summary: string;
  estimated_complexity: string;
  steps: PlanStep[];
}

interface PlanRuntime {
  plan: PipelinePlan;
  step_status: Record<string, string>;
  current_step: string | null;
  artifacts: Record<string, string>;
  error_log: string[];
}

type PlanStatus = 'pending' | 'in_progress' | 'done' | 'failed' | 'skipped';

const STATUS_ICONS: Record<PlanStatus, string> = {
  pending: '⬜',
  in_progress: '🔄',
  done: '✅',
  failed: '❌',
  skipped: '⏭️',
};

export default function PlanPanel() {
  const [runtime, setRuntime] = useState<PlanRuntime | null>(null);
  const [expanded, setExpanded] = useState(true);

  useEffect(() => {
    // Load initial state
    invoke<PlanRuntime | null>('get_pipeline_status').then(setRuntime);

    // Listen for pipeline updates
    const unlisten = listen<PlanRuntime>('pipeline-update', (event) => {
      setRuntime(event.payload);
    });

    return () => {
      unlisten.then((fn) => fn());
    };
  }, []);

  if (!runtime) {
    return (
      <div className="plan-panel plan-panel--empty">
        <span className="plan-panel__title">📋 管道计划</span>
        <span className="plan-panel__hint">无活跃计划</span>
      </div>
    );
  }

  const totalSteps = runtime.plan.steps.length;
  const doneSteps = runtime.plan.steps.filter(
    (s) => runtime.step_status[s.step_id] === 'done' || runtime.step_status[s.step_id] === 'skipped'
  ).length;

  return (
    <div className={`plan-panel ${expanded ? 'plan-panel--expanded' : ''}`}>
      <div className="plan-panel__header" onClick={() => setExpanded(!expanded)}>
        <span className="plan-panel__title">📋 {runtime.plan.summary}</span>
        <span className="plan-panel__progress">
          {doneSteps}/{totalSteps}
        </span>
        <span className="plan-panel__toggle">{expanded ? '▼' : '▶'}</span>
      </div>

      <div className="plan-panel__bar">
        <div
          className="plan-panel__bar-fill"
          style={{ width: `${totalSteps > 0 ? (doneSteps / totalSteps) * 100 : 0}%` }}
        />
      </div>

      {expanded && (
        <div className="plan-panel__steps">
          {runtime.plan.steps.map((step) => {
            const status = (runtime.step_status[step.step_id] || 'pending') as PlanStatus;
            const isCurrent = runtime.current_step === step.step_id;

            return (
              <div
                key={step.step_id}
                className={`plan-panel__step ${isCurrent ? 'plan-panel__step--current' : ''} ${status === 'failed' ? 'plan-panel__step--failed' : ''}`}
              >
                <span className="plan-panel__step-icon">{STATUS_ICONS[status] || '⬜'}</span>
                <span className="plan-panel__step-id">{step.step_id}</span>
                <span className="plan-panel__step-desc">{step.description}</span>
                {step.action === 'approval_gate' && <span className="plan-panel__badge">审批</span>}
                {step.action === 'ask_user' && <span className="plan-panel__badge plan-panel__badge--question">提问</span>}
                {step.action === 'parallel' && <span className="plan-panel__badge plan-panel__badge--parallel">并行</span>}
              </div>
            );
          })}
        </div>
      )}

      {runtime.error_log.length > 0 && (
        <div className="plan-panel__errors">
          {runtime.error_log.map((err, i) => (
            <div key={i} className="plan-panel__error">
              ❌ {err}
            </div>
          ))}
        </div>
      )}
    </div>
  );
}
