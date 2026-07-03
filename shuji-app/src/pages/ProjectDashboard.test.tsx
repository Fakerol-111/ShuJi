import { describe, it, expect, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import ProjectDashboard from '../pages/ProjectDashboard';

vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn().mockResolvedValue(() => {}),
}));

vi.mock('../hooks/useDeptEvents', async (importOriginal) => {
  const actual = await importOriginal<typeof import('../hooks/useDeptEvents')>();
  return {
    ...actual,
    useDeptEvents: () => ({
      activeDepts: new Set<string>(),
      latestLogs: new Map(),
      logEntries: [],
    }),
  };
});

vi.mock('../api', () => {
  const mockResolved = () => Promise.resolve(() => {});
  return {
    onProjectUpdate: vi.fn().mockResolvedValue(mockResolved()),
    onProjectChanged: vi.fn().mockResolvedValue(mockResolved()),
    onDeptLog: vi.fn().mockResolvedValue(mockResolved()),
    onDeptStep: vi.fn().mockResolvedValue(mockResolved()),
    onRuntimeUpdate: vi.fn().mockResolvedValue(mockResolved()),
    onUsageChanged: vi.fn().mockResolvedValue(mockResolved()),
    onChatMessage: vi.fn().mockResolvedValue(mockResolved()),
    onChatDelta: vi.fn().mockResolvedValue(mockResolved()),
    onChatComplete: vi.fn().mockResolvedValue(mockResolved()),
    onPlanUpdate: vi.fn().mockResolvedValue(mockResolved()),
    cancelProcessing: vi.fn().mockResolvedValue(undefined),
    getActiveRoles: vi.fn().mockResolvedValue([]),
    getContextStats: vi.fn().mockResolvedValue({}),
    getRoundMetrics: vi.fn().mockResolvedValue(null),
    getTokenStats: vi.fn().mockResolvedValue({}),
    getWorkflowGraph: vi.fn().mockResolvedValue(null),
    getPipelineStatus: vi.fn().mockResolvedValue(null),
    sendMessage: vi.fn().mockResolvedValue('ok'),
  };
});

vi.mock('../hooks/useProject', () => ({
  useProject: () => ({
    project: null,
    setProject: vi.fn(),
    recentDirs: [],
    setRecentDirs: vi.fn(),
    error: null,
    setError: vi.fn(),
    loadProjectIntoState: vi.fn(),
  }),
}));

vi.mock('../hooks/useChat', () => ({
  useChat: () => ({
    messages: [],
    discussMsgs: [],
    discussing: false,
    tab: 'decision',
    planInfo: null,
    error: null,
    setError: vi.fn(),
    setTab: vi.fn(),
    handleSend: vi.fn(),
    retrySend: vi.fn(),
    handleDiscuss: vi.fn(),
    cancelDiscuss: vi.fn(),
    resetDiscuss: vi.fn(),
    chatEndRef: { current: null },
    setMessages: vi.fn(),
  }),
}));

vi.mock('../hooks/useDocumentTabs', () => ({
  useDocumentTabs: () => ({
    tabs: [],
    activeTab: null,
    openDoc: vi.fn(),
    closeTab: vi.fn(),
    setActiveTab: vi.fn(),
  }),
}));

vi.mock('../hooks/useDemoFlow', () => ({
  useDemoFlow: () => ({
    demoStep: null,
    advanceDemo: vi.fn(),
    resetDemo: vi.fn(),
  }),
}));

vi.mock('../hooks/usePendingApprovals', () => ({
  usePendingApprovals: () => ({
    pendingApprovals: [],
    pipeline: null,
    gateContext: {
      active: false,
      docId: null,
      docType: '',
      stepId: null,
      stepLabel: null,
      stepAction: null,
      nextStepLabel: null,
      planSummary: null,
    },
  }),
}));

vi.mock('../hooks/useProjectPicker', () => ({
  useProjectPicker: () => ({
    pickerOpen: false,
    setPickerOpen: vi.fn(),
    openPicker: vi.fn(),
    handlePick: vi.fn(),
  }),
}));

describe('ProjectDashboard', () => {
  it('renders without project (picker / empty state smoke)', () => {
    render(<ProjectDashboard />);
    expect(screen.getByText(/打开项目/)).toBeTruthy();
  });
});
