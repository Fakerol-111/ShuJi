import { useState, useEffect, useCallback } from 'react';
import { listen } from '@tauri-apps/api/event';
import { classifyError } from '../utils/error';
import { useDeptEvents } from '../hooks/useDeptEvents';
import { useProject } from '../hooks/useProject';
import { useChat } from '../hooks/useChat';
import { useDocumentTabs } from '../hooks/useDocumentTabs';
import { useDemoFlow } from '../hooks/useDemoFlow';
import { usePendingApprovals } from '../hooks/usePendingApprovals';
import { useProjectPicker } from '../hooks/useProjectPicker';
import DashboardLayout from '../components/DashboardLayout';
import AgentStreamPanel from '../components/AgentStreamPanel';
import ArtifactPanel from '../components/ArtifactPanel';
import ProjectPicker from '../components/ProjectPicker';
import SettingsMenu from '../components/SettingsMenu';
import SettingsPage from './SettingsPage';
import HelpDrawer from '../components/HelpDrawer';
import DemoTour from '../components/DemoTour';
import WorkflowGraphView from '../components/WorkflowGraph';

import { Button } from '../components/ui/Button';
import { docIdToPath } from '../utils/docPath';
import type { Project } from '../types';
import type { ActivitySelection } from '../components/ActivityBar';

const STORAGE_KEY = 'shuji_chat';
const STORAGE_UI_KEY = 'shuji_ui_prefs';

function loadSession() {
  try {
    const raw = sessionStorage.getItem(STORAGE_KEY);
    return raw ? JSON.parse(raw) : null;
  } catch {
    return null;
  }
}

function loadUiPrefs() {
  try {
    const raw = localStorage.getItem(STORAGE_UI_KEY);
    return raw ? JSON.parse(raw) : {};
  } catch {
    return {};
  }
}

function saveUiPrefs(prefs: Record<string, unknown>) {
  try {
    localStorage.setItem(STORAGE_UI_KEY, JSON.stringify(prefs));
  } catch {}
}

export default function ProjectDashboard() {
  const session = loadSession();
  const { activeDepts } = useDeptEvents();
  const activeDeptsArr = Array.from(activeDepts);
  const uiPrefs = loadUiPrefs();

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
  const { showDemoTour, setShowDemoTour, demoCreating, mockScenario, handleDemoProject } =
    useDemoFlow(
      project,
      activeDeptsArr,
      planInfo,
      handleSend,
      loadProjectIntoState,
      resetDiscuss,
      setTab,
      setMessages
    );
  const picker = useProjectPicker(loadProjectIntoState, setRecentDirs);

  const [activity, setActivity] = useState<ActivitySelection>(uiPrefs.lastActivity ?? 'files');
  const [uiMode, setUiMode] = useState<'focus' | 'review' | 'inspect'>(
    uiPrefs.lastUiMode || 'focus'
  );
  const [artifactOpen, setArtifactOpen] = useState(false);
  const [settingsOpen, setSettingsOpen] = useState(false);

  // Keyboard shortcuts
  useEffect(() => {
    const onKeyDown = (e: KeyboardEvent) => {
      if (e.key === 'b' && (e.ctrlKey || e.metaKey)) {
        e.preventDefault();
        setActivity((prev) => (prev === 'files' ? null : 'files'));
      }
      if (e.key === '\\' && (e.ctrlKey || e.metaKey)) {
        e.preventDefault();
        setArtifactOpen((prev) => !prev);
      }
      if (e.key === 'Escape' && artifactOpen && uiMode !== 'review') {
        setArtifactOpen(false);
      }
    };
    window.addEventListener('keydown', onKeyDown);
    return () => window.removeEventListener('keydown', onKeyDown);
  }, [artifactOpen, uiMode, setActivity]);

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

  const handleActivity = useCallback(
    (a: ActivitySelection) => {
      if (a === null || a === activity) {
        setActivity(null);
        setUiMode('focus');
      } else if (a === 'graph') {
        setActivity(a);
        setUiMode('inspect');
      } else {
        setActivity(a);
        setUiMode('inspect');
      }
      saveUiPrefs({
        lastUiMode: a === null || a === activity ? 'focus' : 'inspect',
        lastActivity: a === null || a === activity ? null : a,
      });
    },
    [activity]
  );

  const openArtifact = useCallback(
    (path?: string) => {
      if (path) openTab(path);
      setArtifactOpen(true);
    },
    [openTab]
  );

  useEffect(() => {
    if (hasTabs) setArtifactOpen(true);
  }, [hasTabs]);

  useEffect(() => {
    if (pendingApprovals.length === 0) return;
    setUiMode('review');
    setArtifactOpen(true);
    openTab(docIdToPath(pendingApprovals[0]));
  }, [pendingApprovals, openTab]);

  const handlePendingApproval = useCallback(
    (docPath: string) => {
      openTab(docPath);
      setArtifactOpen(true);
    },
    [openTab]
  );

  const projectPhases = project?.phases || [];
  const phaseCount = project?.phase_count || 0;
  const overall =
    typeof project?.overall === 'string'
      ? project.overall
      : String(project?.overall || 'NotStarted');

  const graphContent = activity === 'graph' ? <WorkflowGraphView /> : null;

  return (
    <>
      <DashboardLayout
        project={project}
        error={error}
        clearError={clearError}
        activity={activity}
        onActivity={handleActivity}
        activeDocPath={activeDoc?.path || null}
        onDocSelect={handleDocSelect}
        onShowDiff={(path) => openTab(path, 'diff')}
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
            <SettingsMenu onOpenSettings={() => setSettingsOpen(true)} />
          </>
        }
        agentStream={
          activity === 'graph' ? null : (
            <AgentStreamPanel
              project={project}
              tab={tab}
              messages={messages}
              discussMsgs={discussMsgs}
              discussing={discussing}
              planInfo={planInfo}
              activeDeptsCount={activeDeptsArr.length}
              activeDepts={activeDeptsArr}
              pendingApprovals={pendingApprovals}
              phases={projectPhases}
              phaseCount={phaseCount}
              overall={overall}
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
              onSelectDoc={(path) => openArtifact(path)}
              onOpenProject={picker.openPicker}
              endRef={chatEndRef}
            />
          )
        }
        artifactPanel={
          activity === 'graph' ? (
            graphContent
          ) : project ? (
            <ArtifactPanel
              project={project}
              tabs={tabs}
              activeIndex={activeIndex}
              activeDoc={activeDoc || null}
              hasTabs={hasTabs}
              pendingApprovals={pendingApprovals}
              onSelectTab={setActiveIndex}
              onCloseTab={closeTab}
              onClosePanel={() => setArtifactOpen(false)}
              onOpenApproval={handlePendingApproval}
            />
          ) : undefined
        }
        artifactOpen={artifactOpen}
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
      {settingsOpen && (
        <div className="fixed inset-0 z-50 bg-surface-paper overflow-y-auto">
          <SettingsPage onClose={() => setSettingsOpen(false)} />
        </div>
      )}
    </>
  );
}
