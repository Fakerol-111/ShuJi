import { describe, expect, it } from 'vitest';
import { buildRecentDocIds, buildTimelineNodes, computeNextAction } from './workflowTimeline';
import type { PipelineRuntime } from '../types';

const labels = {
  waitingApproval: (doc: string) => `approve ${doc}`,
  waitingInput: (step: string) => `input ${step}`,
  waitingApprovalGate: 'gate',
  deptWorking: (dept: string) => `${dept} working`,
  pipelineRunning: 'running',
};

const pipeline: PipelineRuntime = {
  plan: {
    plan_id: 'p1',
    summary: 'test plan',
    estimated_complexity: 'low',
    steps: [
      {
        step_id: 's1',
        description: 'Design',
        action: 'route_to',
        action_params: { target: '中书令' },
        depends_on: [],
        require_approval: false,
      },
      {
        step_id: 's2',
        description: 'Review gate',
        action: 'approval_gate',
        action_params: {},
        depends_on: ['s1'],
        require_approval: true,
      },
    ],
  },
  step_status: { s1: 'done', s2: 'in_progress' },
  current_step: 's2',
  artifacts: { s1: 'dsgn_1' },
  error_log: [],
};

describe('buildTimelineNodes', () => {
  it('maps pipeline steps with statuses', () => {
    const nodes = buildTimelineNodes(pipeline, null, null, ['revw_1']);
    expect(nodes).toHaveLength(2);
    expect(nodes[0].status).toBe('done');
    expect(nodes[0].docId).toBe('dsgn_1');
    expect(nodes[1].status).toBe('waiting');
  });
});

describe('buildRecentDocIds', () => {
  it('prioritizes pending approvals then artifacts', () => {
    const ids = buildRecentDocIds(pipeline, null, ['revw_1'], 3);
    expect(ids[0]).toBe('revw_1');
    expect(ids).toContain('dsgn_1');
  });
});

describe('computeNextAction', () => {
  it('returns approval when pending docs exist', () => {
    const action = computeNextAction([], ['revw_1'], null, labels);
    expect(action?.type).toBe('approval');
    expect(action?.docId).toBe('revw_1');
  });

  it('returns running when dept active', () => {
    const action = computeNextAction(['工部'], [], null, labels);
    expect(action?.type).toBe('running');
    expect(action?.dept).toBe('工部');
  });
});
