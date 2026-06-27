import { describe, it, expect, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import type { ComponentProps } from 'react';
import AgentStreamPanel from './AgentStreamPanel';
import type { PhaseRuntime, Project } from '../types';

vi.mock('./DeptCardRail', () => ({
  default: () => <div data-testid="dept-card-rail" />,
}));

vi.mock('./ChatPanel', () => ({
  default: () => <div data-testid="chat-panel" />,
}));

vi.mock('./ProjectOverview', () => ({
  default: () => <div data-testid="project-overview" />,
}));

vi.mock('../hooks/useDeptEvents', () => ({
  useDeptEvents: () => ({
    latestLogs: new Map(),
    logEntries: [],
  }),
}));

vi.mock('../hooks/useDeliveryValidation', () => ({
  useDeliveryValidation: () => ({
    report: null,
    loading: false,
    refresh: vi.fn(),
  }),
}));

vi.mock('../hooks/useWorkflowTimeline', () => ({
  useWorkflowTimeline: () => ({
    wfState: {
      profile_id: 'greenfield_standard',
      chain_id: 'c1',
      current_stage: 'approval',
      stage_status: 'pending',
    },
    timelineNodes: [],
    recentDocIds: [],
    nextAction: { type: 'approval', label: '等待朱批 revw_001' },
    pipelineProgress: { done: 1, total: 3, summary: 'test' },
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
  }),
}));

vi.mock('../api', () => ({
  getRoundMetrics: vi.fn().mockResolvedValue(null),
}));

vi.mock('./PlanPanel', () => ({
  default: () => null,
}));

const phases: PhaseRuntime[] = [{ index: 0, design: 'PendingApproval', execution: 'NotStarted' }];

const project: Project = {
  id: 'p1',
  name: 'Test',
  goal: 'goal',
  working_dir: '/tmp/test',
  overall: 'PendingApproval',
  phases,
  phase_count: 1,
  created_at: '2026-06-26T00:00:00Z',
  updated_at: '2026-06-26T00:00:00Z',
};

const baseProps: ComponentProps<typeof AgentStreamPanel> = {
  project,
  tab: 'decision',
  setTab: () => {},
  messages: [],
  discussMsgs: [],
  discussing: false,
  planInfo: null,
  activeDeptsCount: 0,
  activeDepts: ['gongbushangshu'],
  pendingApprovals: ['revw_001'],
  phases,
  phaseCount: 1,
  overall: 'PendingApproval',
  onOption: () => {},
  onSend: () => {},
  onRetrySend: async () => {},
  onDiscuss: () => {},
  onCancelDiscuss: () => {},
  onConvertToCommand: () => {},
  onSelectDoc: () => {},
  endRef: { current: null },
};

describe('AgentStreamPanel', () => {
  it('shows pending approval in command bar', () => {
    render(<AgentStreamPanel {...baseProps} />);
    expect(screen.getByText(/等待朱批 revw_001/)).toBeTruthy();
    expect(screen.getByText(/待陛下朱批/)).toBeTruthy();
  });

  it('hides dept card rail in beginner mode', () => {
    render(<AgentStreamPanel {...baseProps} beginnerMode />);
    expect(screen.queryByTestId('dept-card-rail')).toBeNull();
  });

  it('shows dept card rail in advanced mode', () => {
    render(<AgentStreamPanel {...baseProps} beginnerMode={false} />);
    expect(screen.getByTestId('dept-card-rail')).toBeTruthy();
  });
});
