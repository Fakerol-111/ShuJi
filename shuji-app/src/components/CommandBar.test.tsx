import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';
import CommandBar from './CommandBar';
import type { PhaseRuntime } from '../types';

vi.mock('../hooks/useDeptEvents', () => ({
  useDeptEvents: () => ({ latestLogs: new Map() }),
}));

const activeTimeline = {
  wfState: {
    profile_id: 'greenfield_standard',
    chain_id: 'chain_1',
    current_stage: 'design',
    stage_status: 'in_progress',
  },
  timelineNodes: [],
  recentDocIds: [],
  nextAction: { type: 'approval' as const, label: '等待朱批 revw_001' },
  pipelineProgress: { done: 2, total: 5, summary: 'Pipeline test' },
  gongbuBatch: null,
  hasFlowActivity: true,
  pipeline: {
    plan: {
      plan_id: 'p1',
      summary: 'test',
      estimated_complexity: 'low',
      steps: [{ step_id: 's1', description: 'step', action: 'route_to', action_params: {} }],
    },
    step_status: { s1: 'pending' },
    current_step: 's1',
    artifacts: {},
    error_log: [],
  },
};

const idleTimeline = {
  wfState: null,
  timelineNodes: [],
  recentDocIds: [],
  nextAction: null,
  pipelineProgress: null,
  gongbuBatch: null,
  hasFlowActivity: false,
  pipeline: null,
};

let timelineState = activeTimeline;

vi.mock('../hooks/useWorkflowTimeline', () => ({
  useWorkflowTimeline: () => timelineState,
}));

vi.mock('../api', () => ({
  getRoundMetrics: vi.fn().mockResolvedValue(null),
}));

vi.mock('./PlanPanel', () => ({
  default: () => null,
}));

const phases: PhaseRuntime[] = [{ index: 0, design: 'Approved', execution: 'Implementing' }];

describe('CommandBar', () => {
  beforeEach(() => {
    localStorage.clear();
    timelineState = activeTimeline;
  });

  it('renders idle state when no workflow activity', () => {
    timelineState = idleTimeline;
    render(
      <CommandBar
        totalStageCount={0}
        completedStageCount={0}
        phaseCount={0}
        phases={[]}
        overall="NotStarted"
        activeDepts={[]}
        planInfo={null}
        pendingApprovals={[]}
        onSelectDoc={() => {}}
      />
    );
    expect(screen.getByText(/尚未启奏/i)).toBeTruthy();
  });

  it('shows active dept, next action, and pending approval badge', () => {
    render(
      <CommandBar
        totalStageCount={1}
        completedStageCount={0}
        phaseCount={1}
        phases={phases}
        overall="Approved"
        activeDepts={['gongbushangshu']}
        planInfo={null}
        pendingApprovals={['revw_001']}
        onSelectDoc={() => {}}
      />
    );
    expect(screen.getByText(/等待朱批 revw_001/)).toBeTruthy();
    expect(screen.getByText(/待陛下朱批/)).toBeTruthy();
    expect(screen.getByText(/工部/)).toBeTruthy();
  });

  it('renders validation summary when report provided', async () => {
    render(
      <CommandBar
        totalStageCount={1}
        completedStageCount={1}
        phaseCount={1}
        phases={phases}
        overall="Approved"
        activeDepts={[]}
        planInfo={null}
        pendingApprovals={[]}
        validationReport={{
          ts: '2026-06-26T00:00:00Z',
          project_type: 'rust',
          overall_pass: true,
          checks: [{ name: 'tests', pass: true, summary: 'ok', details: {} }],
          ctrt_id: null,
        }}
        onSelectDoc={() => {}}
      />
    );
    await waitFor(() => {
      expect(screen.getByText(/验证通过/i)).toBeTruthy();
    });
  });
});
