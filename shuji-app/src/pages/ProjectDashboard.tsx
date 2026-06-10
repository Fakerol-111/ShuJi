import { useState, useEffect, useCallback } from 'react';
import { listen } from '@tauri-apps/api/event';
import { formatError, classifyError } from '../utils/error';
import { open } from '@tauri-apps/plugin-dialog';
import { getRecentDirs } from '../api';
import { useActiveDepts } from '../hooks/useActiveDepts';
import { useProject } from '../hooks/useProject';
import { useChat, type Tab } from '../hooks/useChat';
import { useDocumentTabs } from '../hooks/useDocumentTabs';
import { useDemoFlow } from '../hooks/useDemoFlow';
import { usePendingApprovals } from '../hooks/usePendingApprovals';
import DashboardLayout from '../components/DashboardLayout';
import ActiveDeptStrip from '../components/ActiveDeptStrip';
import DemoSummaryCard from '../components/DemoSummaryCard';
import DocPreview from '../components/DocPreview';
import ProjectOverview from '../components/ProjectOverview';
import SettingsMenu from '../components/SettingsMenu';
import HelpDrawer from '../components/HelpDrawer';
import ProjectPicker from '../components/ProjectPicker';
import ChatPanel from '../components/ChatPanel';
import DemoTour from '../components/DemoTour';
import { Tabs } from '../components/ui/Tabs';
import { Button } from '../components/ui/Button';
import TabBar from '../components/TabBar';
import WorkflowStatus from '../components/WorkflowTimeline';
import WorkflowGraphView from '../components/WorkflowGraph';
import type { Project } from '../types';
import type { ActivitySelection } from '../components/ActivityBar';

const STORAGE_KEY = 'shuji_chat';
const CHAT_PANEL_MIN = 300;
const CHAT_PANEL_MAX = 600;

function loadSession() {
  try {
    const raw = sessionStorage.getItem(STORAGE_KEY);
    return raw ? JSON.parse(raw) : null;
  } catch {
    return null;
  }
}

export default function ProjectDashboard() {
  const session = loadSession();
  const activeDepts = Array.from(useActiveDepts());

  // ── Project state ──
  const {
    project,
    setProject,
    recentDirs,
    setRecentDirs,
    error: projError,
    setError: setProjError,
    loadProjectIntoState,
  } = useProject();

  // ── Chat state ──
  const {
    messages,
    discussMsgs,
    discussing,
    tab,
    planInfo,
    error: chatError,
    setError: setChatError,
    setTab,
    handleSend,
    retrySend,
    handleDiscuss,
    cancelDiscuss,
    resetDiscuss,
    chatEndRef,
  } = useChat(session?.msgs || []);

  // ── Document tabs ──
  const {
    tabs,
    activeIndex,
    activeDoc,
    hasTabs,
    openTab,
    closeTab,
    handleDocSelect,
    setActiveIndex,
  } = useDocumentTabs();

  // ── Pending approvals ──
  const { pendingApprovals } = usePendingApprovals(project);

  // ── Demo flow ──
  const { showDemoTour, setShowDemoTour, demoCreating, demoSummary, handleDemoProject } =
    useDemoFlow(
      project,
      activeDepts,
      planInfo,
      handleSend,
      loadProjectIntoState,
      resetDiscuss,
      setTab
    );

  // ── Session persistence ──
  useEffect(() => {
    try {
      sessionStorage.setItem(STORAGE_KEY, JSON.stringify({ msgs: messages, discuss: discussMsgs }));
    } catch {}
  }, [messages, discussMsgs]);

  // ── Project-update events ──
  useEffect(() => {
    const unlisten = listen('project-update', (event: { payload: Project }) => {
      setProject(event.payload);
    });
    return () => {
      unlisten.then((f) => f());
    };
  }, [setProject]);

  // ── Error handling ──
  const error = projError || chatError;
  const clearError = useCallback(() => {
    setProjError('');
    setChatError('');
  }, [setProjError, setChatError]);
  useEffect(() => {
    if (!error || classifyError(error) === 'critical') return;
    const timer = setTimeout(clearError, 8000);
    return () => clearTimeout(timer);
  }, [error, clearError]);

  // ── UI state ──
  const [activity, setActivity] = useState<ActivitySelection>('files');
  const [logsExpanded, setLogsExpanded] = useState(false);
  const [chatWidth, setChatWidth] = useState(400);
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [showPicker, setShowPicker] = useState(false);
  const [pickerPath, setPickerPath] = useState('');
  const [pickerLoading, setPickerLoading] = useState(false);
  const [pickerError, setPickerError] = useState('');

  const openProjectPicker = () => {
    setPickerPath('');
    setPickerError('');
    getRecentDirs()
      .then(setRecentDirs)
      .catch((e) => setPickerError(formatError(e)));
    setShowPicker(true);
  };

  const handleBrowse = async () => {
    try {
      const selected = await open({ directory: true, multiple: false, title: '选择工作目录' });
      if (selected) setPickerPath(selected);
    } catch (e) {
      setPickerError(formatError(e));
    }
  };

  const handleLoadProject = async (dir?: string) => {
    const path = dir || pickerPath.trim();
    if (!path) {
      setPickerError('请选择工作目录');
      return;
    }
    setPickerLoading(true);
    setPickerError('');
    try {
      await loadProjectIntoState(path);
      sessionStorage.removeItem(STORAGE_KEY);
      setShowPicker(false);
    } catch (e) {
      setPickerError(formatError(e));
    } finally {
      setPickerLoading(false);
    }
  };

  const startResize = (e: React.MouseEvent) => {
    e.preventDefault();
    const startX = e.clientX;
    const startWidth = chatWidth;
    const move = (ev: MouseEvent) =>
      setChatWidth(
        Math.max(CHAT_PANEL_MIN, Math.min(CHAT_PANEL_MAX, startWidth - (ev.clientX - startX)))
      );
    const up = () => {
      document.removeEventListener('mousemove', move);
      document.removeEventListener('mouseup', up);
    };
    document.addEventListener('mousemove', move);
    document.addEventListener('mouseup', up);
  };

  const handleConvertToCommand = (text: string) => {
    handleSend(text);
    setTab('decision');
  };

  // ── Main content ──
  const mainContent = (
    <>
      <ActiveDeptStrip activeDepts={activeDepts} planInfo={planInfo} />
      {project && (
        <WorkflowStatus
          phaseCount={project.phase_count}
          phases={project.phases}
          overall={typeof project.overall === 'string' ? project.overall : String(project.overall)}
          activeDepts={activeDepts}
          planInfo={planInfo}
          pendingApprovals={pendingApprovals}
          onSelectDoc={(path) => openTab(path)}
        />
      )}
      <div className="flex-1 min-h-0 overflow-hidden flex flex-col">
        {activity === 'graph' ? (
          <WorkflowGraphView />
        ) : demoSummary ? (
          <DemoSummaryCard summary={demoSummary} onOpenProject={openProjectPicker} />
        ) : (
          <>
            {hasTabs && (
              <TabBar
                tabs={tabs}
                activeIndex={activeIndex}
                onSelect={setActiveIndex}
                onClose={closeTab}
              />
            )}
            {hasTabs ? (
              <DocPreview
                key={activeDoc!.path}
                projectDir={project!.working_dir}
                docPath={activeDoc!.path}
                initialTab={activeDoc!.initialView}
                onClose={() => closeTab(activeIndex)}
              />
            ) : (
              <ProjectOverview
                project={project}
                activeDepts={activeDepts}
                planInfo={planInfo}
                onOpenProject={openProjectPicker}
                onDocSelect={(path) => openTab(path)}
              />
            )}
          </>
        )}
      </div>
    </>
  );

  const chatPanel = (
    <section
      className="relative bg-surface-paper border-l border-fold flex flex-col min-h-0 shrink-0"
      style={{ width: chatWidth }}
    >
      <div
        onMouseDown={startResize}
        className="absolute left-0 top-0 bottom-0 w-1 cursor-col-resize hover:bg-vermillion/40 transition-colors"
      />
      <div className="border-b border-fold bg-surface-elevated shrink-0 px-3 py-2">
        <Tabs
          tabs={[
            { key: 'decision', label: '决策' },
            { key: 'discuss', label: '廷议' },
          ]}
          activeKey={tab}
          onChange={(k) => setTab(k as Tab)}
        />
        <div className="text-ui text-ink-600 mt-1">
          {tab === 'decision' ? '下达敕令，驱动各部门执行' : '仅与内阁议政，不改代码、不写文档'}
        </div>
      </div>
      {!project ? (
        <div className="flex-1 flex items-center justify-center text-body text-ink-400">
          请先开卷
        </div>
      ) : (
        <ChatPanel
          tab={tab}
          messages={messages}
          discussMsgs={discussMsgs}
          discussing={discussing}
          planInfo={planInfo}
          activeDeptsCount={activeDepts.length}
          onOption={(key, supplement) => handleSend(supplement ? `${key}\n${supplement}` : key)}
          onSend={handleSend}
          onRetrySend={retrySend}
          onDiscuss={handleDiscuss}
          onCancelDiscuss={cancelDiscuss}
          onConvertToCommand={handleConvertToCommand}
          endRef={chatEndRef}
        />
      )}
    </section>
  );

  return (
    <DashboardLayout
      project={project}
      error={error}
      clearError={clearError}
      activity={activity}
      onActivity={setActivity}
      activeDocPath={activeDoc?.path || null}
      onDocSelect={handleDocSelect}
      onShowDiff={(path) => openTab(path, 'diff')}
      logsExpanded={logsExpanded}
      onLogsExpanded={setLogsExpanded}
      pendingApprovalsCount={pendingApprovals.length}
      headerRight={
        <>
          <Button
            variant="seal"
            className="text-xs !px-2 !py-1"
            onClick={handleDemoProject}
            disabled={demoCreating}
          >
            {demoCreating ? '创建中…' : '体验枢机'}
          </Button>
          <Button
            variant="ghost"
            className="text-xs !px-2 !py-1 text-ink-400"
            onClick={openProjectPicker}
          >
            打开项目
          </Button>
          <HelpDrawer />
          <SettingsMenu open={settingsOpen} setOpen={setSettingsOpen} />
        </>
      }
      mainContent={mainContent}
      chatPanel={chatPanel}
      picker={
        showPicker ? (
          <ProjectPicker
            recentDirs={recentDirs}
            pickerPath={pickerPath}
            pickerError={pickerError}
            pickerLoading={pickerLoading}
            setPickerPath={setPickerPath}
            onBrowse={handleBrowse}
            onLoad={handleLoadProject}
            onClose={() => setShowPicker(false)}
          />
        ) : undefined
      }
      demoTour={showDemoTour ? <DemoTour onClose={() => setShowDemoTour(false)} /> : undefined}
    />
  );
}
