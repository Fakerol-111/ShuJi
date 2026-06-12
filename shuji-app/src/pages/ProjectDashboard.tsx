import { useState, useEffect, useCallback } from 'react';
import { listen } from '@tauri-apps/api/event';
import { classifyError } from '../utils/error';
import { useActiveDepts } from '../hooks/useActiveDepts';
import { useProject } from '../hooks/useProject';
import { useChat } from '../hooks/useChat';
import { useDocumentTabs } from '../hooks/useDocumentTabs';
import { useDemoFlow } from '../hooks/useDemoFlow';
import { usePendingApprovals } from '../hooks/usePendingApprovals';
import { useProjectPicker } from '../hooks/useProjectPicker';
import DashboardLayout from '../components/DashboardLayout';
import DashboardMainContent from '../components/DashboardMainContent';
import DashboardChatPanel from '../components/DashboardChatPanel';
import ProjectPicker from '../components/ProjectPicker';
import SettingsMenu from '../components/SettingsMenu';
import HelpDrawer from '../components/HelpDrawer';
import DemoTour from '../components/DemoTour';
import { Button } from '../components/ui/Button';
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

  const {
    project,
    setProject,
    recentDirs,
    setRecentDirs,
    error: projError,
    setError: setProjError,
    loadProjectIntoState,
  } = useProject();

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
    setMessages,
  } = useChat(session?.msgs || []);

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

  const { pendingApprovals } = usePendingApprovals(project);
  const {
    showDemoTour,
    setShowDemoTour,
    demoCreating,
    demoSummary,
    mockScenario,
    handleDemoProject,
  } = useDemoFlow(
    project,
    activeDepts,
    planInfo,
    handleSend,
    loadProjectIntoState,
    resetDiscuss,
    setTab,
    setMessages
  );
  const picker = useProjectPicker(loadProjectIntoState, setRecentDirs);

  const [activity, setActivity] = useState<ActivitySelection>('files');
  const [logsExpanded, setLogsExpanded] = useState(false);
  const [chatWidth, setChatWidth] = useState(400);
  useEffect(() => {
    try {
      sessionStorage.setItem(STORAGE_KEY, JSON.stringify({ msgs: messages, discuss: discussMsgs }));
    } catch {}
  }, [messages, discussMsgs]);

  useEffect(() => {
    const unlisten = listen('project-update', (event: { payload: Project }) => {
      setProject(event.payload);
    });
    return () => {
      unlisten.then((f) => f());
    };
  }, [setProject]);

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
            onClick={picker.openPicker}
          >
            打开项目
          </Button>
          <HelpDrawer />
          <SettingsMenu />
        </>
      }
      mainContent={
        <DashboardMainContent
          project={project}
          activeDepts={activeDepts}
          planInfo={planInfo}
          activity={activity}
          pendingApprovals={pendingApprovals}
          demoSummary={demoSummary}
          tabs={tabs}
          activeIndex={activeIndex}
          activeDoc={activeDoc || null}
          hasTabs={hasTabs}
          setActiveIndex={setActiveIndex}
          closeTab={closeTab}
          openTab={openTab}
          onOpenProject={picker.openPicker}
        />
      }
      chatPanel={
        <DashboardChatPanel
          chatWidth={chatWidth}
          project={project}
          tab={tab}
          messages={messages}
          discussMsgs={discussMsgs}
          discussing={discussing}
          planInfo={planInfo}
          activeDeptsCount={activeDepts.length}
          setTab={setTab}
          onOption={(k, s) => handleSend(s ? `${k}\n${s}` : k)}
          onSend={handleSend}
          onRetrySend={retrySend}
          onDiscuss={handleDiscuss}
          onCancelDiscuss={cancelDiscuss}
          onConvertToCommand={(t) => {
            handleSend(t);
            setTab('decision');
          }}
          endRef={chatEndRef}
          onResizeStart={startResize}
        />
      }
      picker={
        picker.showPicker ? (
          <ProjectPicker
            recentDirs={recentDirs}
            pickerPath={picker.pickerPath}
            pickerError={picker.pickerError}
            pickerLoading={picker.pickerLoading}
            setPickerPath={picker.setPickerPath}
            onBrowse={picker.onBrowse}
            onLoad={picker.onLoad}
            onClose={() => picker.setShowPicker(false)}
          />
        ) : undefined
      }
      demoTour={
        showDemoTour ? (
          <DemoTour onClose={() => setShowDemoTour(false)} mockMode={!!mockScenario} />
        ) : undefined
      }
    />
  );
}
