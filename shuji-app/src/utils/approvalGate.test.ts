import { describe, it, expect } from 'vitest';
import { computeApprovalGateContext } from './approvalGate';
import type { PipelineRuntime } from '../types';

const pipeline: PipelineRuntime = {
  plan: {
    plan_id: 'p1',
    summary: 'Test plan',
    estimated_complexity: 'medium',
    steps: [
      {
        step_id: 'review',
        description: '门下审查',
        action: 'route_to',
        action_params: {},
        depends_on: [],
        require_approval: false,
      },
      {
        step_id: 'gate',
        description: '等待朱批',
        action: 'approval_gate',
        action_params: {},
        depends_on: ['review'],
        require_approval: true,
      },
      {
        step_id: 'execute',
        description: '尚书令执行',
        action: 'route_to',
        action_params: {},
        depends_on: ['gate'],
        require_approval: false,
      },
    ],
  },
  step_status: { review: 'done', gate: 'in_progress' },
  current_step: 'gate',
  artifacts: { review: 'revw_001', gate: 'revw_001' },
  error_log: [],
};

describe('computeApprovalGateContext', () => {
  it('returns inactive when no pending docs', () => {
    const ctx = computeApprovalGateContext([], pipeline);
    expect(ctx.active).toBe(false);
    expect(ctx.docId).toBeNull();
  });

  it('derives doc type and gate step from pipeline', () => {
    const ctx = computeApprovalGateContext(['revw_001'], pipeline);
    expect(ctx.active).toBe(true);
    expect(ctx.docId).toBe('revw_001');
    expect(ctx.docType).toBe('revw');
    expect(ctx.stepId).toBe('gate');
    expect(ctx.stepLabel).toBe('等待朱批');
    expect(ctx.nextStepLabel).toBe('尚书令执行');
  });

  it('works without pipeline runtime', () => {
    const ctx = computeApprovalGateContext(['revw_002'], null);
    expect(ctx.active).toBe(true);
    expect(ctx.docId).toBe('revw_002');
    expect(ctx.stepId).toBeNull();
  });
});
