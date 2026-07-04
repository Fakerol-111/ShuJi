import { describe, it, expect, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import { BrowserRouter } from 'react-router-dom';
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
    getPendingApprovals: vi.fn().mockResolvedValue([]),
    sendMessage: vi.fn().mockResolvedValue('ok'),
    getConfig: vi.fn().mockResolvedValue({ roles: { default: { api_key: 'test' } } }),
    loadProject: vi.fn().mockResolvedValue(null),
    getRecentDirs: vi.fn().mockResolvedValue([]),
    getChatHistory: vi.fn().mockResolvedValue([]),
  };
});

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
    activeIndex: 0,
    activeDoc: null,
    hasTabs: false,
    openTab: vi.fn(),
    closeTab: vi.fn(),
    handleDocSelect: vi.fn(),
    setActiveIndex: vi.fn(),
  }),
}));

vi.mock('../hooks/useDemoFlow', () => ({
  useDemoFlow: () => ({
    showDemoTour: false,
    setShowDemoTour: vi.fn(),
    demoCreating: false,
    mockScenario: null,
    handleDemoProject: vi.fn(),
  }),
}));

vi.mock('../hooks/useProjectPicker', () => ({
  useProjectPicker: () => ({
    showPicker: false,
    pickerPath: '',
    pickerError: '',
    pickerLoading: false,
    setPickerPath: vi.fn(),
    onBrowse: vi.fn(),
    onLoad: vi.fn(),
    setShowPicker: vi.fn(),
    openPicker: vi.fn(),
  }),
}));

vi.mock('../hooks/useDashboardUI', () => ({
  useDashboardUI: () => ({
    activity: 'files' as const,
    onActivity: vi.fn(),
    experienceLevel: 'advanced' as const,
    onExperienceLevelChange: vi.fn(),
    beginnerMode: false,
    artifactOpen: false,
    setArtifactOpen: vi.fn(),
    settingsOpen: false,
    setSettingsOpen: vi.fn(),
    showProjectOnboarding: false,
    setShowProjectOnboarding: vi.fn(),
    openArtifact: vi.fn(),
  }),
}));

describe('ProjectDashboard', () => {
  it('renders without project (picker / empty state smoke)', () => {
    render(
      <BrowserRouter>
        <ProjectDashboard />
      </BrowserRouter>
    );
    expect(screen.getByText(/打开项目/)).toBeTruthy();
  });
});
