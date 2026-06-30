import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { render } from '@testing-library/react';
import DashboardLayout from './DashboardLayout';
import type { Project } from '../types';

vi.mock('../hooks/useDeptEvents', () => ({
  DeptEventsProvider: ({ children }: { children: React.ReactNode }) => <div>{children}</div>,
  useDeptEvents: () => ({ latestLogs: new Map(), logEntries: [], deptSteps: new Map() }),
}));

vi.mock('../hooks/useUsageStats', () => ({
  UsageStatsProvider: ({ children }: { children: React.ReactNode }) => <div>{children}</div>,
  useUsageStats: () => ({ tokenStats: {}, roundMetrics: null }),
}));

vi.mock('./ActivityBar', () => ({
  default: () => <div data-testid="activity-bar" />,
}));

vi.mock('./Sidebar', () => ({
  default: () => <div data-testid="sidebar" />,
}));

vi.mock('./DutyBar', () => ({
  default: () => <div data-testid="duty-bar" />,
}));

vi.mock('./SealLogo', () => ({
  SealLogo: () => <div data-testid="seal-logo" />,
}));

const project: Project = {
  id: 'p1',
  name: 'Test',
  goal: '',
  working_dir: '/tmp/test',
  overall: 'NotStarted',
  phases: [],
  phase_count: 0,
  created_at: '',
  updated_at: '',
};

const baseProps = {
  project,
  error: '',
  clearError: vi.fn(),
  activity: null as null,
  onActivity: vi.fn(),
  activeDocPath: null,
  onDocSelect: vi.fn(),
  onShowDiff: vi.fn(),
  pendingApprovalsCount: 0,
  agentStream: <div data-testid="agent-stream">stream</div>,
  artifactPanel: <div data-testid="artifact-panel">panel</div>,
  artifactOpen: true,
};

describe('DashboardLayout', () => {
  beforeEach(() => {
    localStorage.clear();
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  it('loads default artifact width when localStorage is empty', () => {
    render(<DashboardLayout {...baseProps} />);
    const aside = document.querySelector('aside');
    expect(aside).toBeTruthy();
    expect(aside!.style.width).toBe('520px');
  });

  it('loads saved artifact width from localStorage', () => {
    localStorage.setItem('shuji_artifact_width', '400');
    render(<DashboardLayout {...baseProps} />);
    const aside = document.querySelector('aside');
    expect(aside!.style.width).toBe('400px');
  });

  it('does not render aside when artifactOpen is false', () => {
    render(<DashboardLayout {...baseProps} artifactOpen={false} />);
    expect(document.querySelector('aside')).toBeNull();
  });

  it('renders drag handle with pointer events (no onMouseDown)', () => {
    render(<DashboardLayout {...baseProps} />);
    const handle = document.querySelector('.cursor-col-resize');
    expect(handle).toBeTruthy();
    // Should use pointer events, not mouse events
    expect(handle!.getAttribute('onmousedown')).toBeNull();
  });

  it('inner flex containers have min-w-0 and overflow-hidden', () => {
    const { container } = render(<DashboardLayout {...baseProps} />);
    const mainRow = container.querySelector('.flex-1.flex.min-h-0.min-w-0.overflow-hidden');
    expect(mainRow).toBeTruthy();
    const innerContainer = container.querySelector('.flex-1.flex.min-w-0.min-h-0.overflow-hidden');
    expect(innerContainer).toBeTruthy();
  });

  it('aside has maxWidth set to ARTIFACT_MAX', () => {
    render(<DashboardLayout {...baseProps} />);
    const aside = document.querySelector('aside');
    expect(aside!.style.maxWidth).toBe('680px');
  });
});
