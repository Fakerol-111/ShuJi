import type { PipelineRuntime } from '../types';

export interface ApprovalGateContext {
  active: boolean;
  docId: string | null;
  docType: string;
  stepId: string | null;
  stepLabel: string | null;
  stepAction: string | null;
  nextStepLabel: string | null;
  planSummary: string | null;
}

function findNextStepLabel(
  pipeline: PipelineRuntime | null,
  afterStepId: string | null
): string | null {
  if (!pipeline || !afterStepId) return null;
  const idx = pipeline.plan.steps.findIndex((s) => s.step_id === afterStepId);
  if (idx < 0) return null;
  for (let i = idx + 1; i < pipeline.plan.steps.length; i++) {
    const step = pipeline.plan.steps[i];
    const st = pipeline.step_status[step.step_id];
    if (st === 'done' || st === 'skipped') continue;
    return step.description || step.step_id;
  }
  return null;
}

function findWaitingGateStep(
  pipeline: PipelineRuntime,
  docId: string
): { stepId: string; stepLabel: string; stepAction: string } | null {
  for (const step of pipeline.plan.steps) {
    if (step.action !== 'approval_gate') continue;
    const st = pipeline.step_status[step.step_id];
    if (st === 'done' || st === 'skipped') continue;
    const artifact = pipeline.artifacts[step.step_id];
    if (artifact === docId || !artifact) {
      return {
        stepId: step.step_id,
        stepLabel: step.description || step.step_id,
        stepAction: step.action,
      };
    }
  }
  return null;
}

export function computeApprovalGateContext(
  pendingApprovals: string[],
  pipeline: PipelineRuntime | null
): ApprovalGateContext {
  const docId = pendingApprovals[0] ?? null;
  if (!docId) {
    return {
      active: false,
      docId: null,
      docType: '',
      stepId: null,
      stepLabel: null,
      stepAction: null,
      nextStepLabel: null,
      planSummary: pipeline?.plan.summary ?? null,
    };
  }

  const docType = docId.split('_')[0] || 'revw';
  let stepId = pipeline?.current_step ?? null;
  let stepLabel: string | null = null;
  let stepAction: string | null = null;

  if (pipeline) {
    const current = stepId ? pipeline.plan.steps.find((s) => s.step_id === stepId) : undefined;
    if (current?.action === 'approval_gate') {
      stepLabel = current.description || current.step_id;
      stepAction = current.action;
    } else {
      const gate = findWaitingGateStep(pipeline, docId);
      if (gate) {
        stepId = gate.stepId;
        stepLabel = gate.stepLabel;
        stepAction = gate.stepAction;
      } else if (current) {
        stepLabel = current.description || current.step_id;
        stepAction = current.action;
      }
    }
  }

  return {
    active: true,
    docId,
    docType,
    stepId,
    stepLabel,
    stepAction,
    nextStepLabel: findNextStepLabel(pipeline, stepId),
    planSummary: pipeline?.plan.summary ?? null,
  };
}
