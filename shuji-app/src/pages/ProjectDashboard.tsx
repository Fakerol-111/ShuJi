import { useEffect, useCallback } from 'react';
import { classifyError, swallowError } from '../utils/error';
import { useDeptEvents } from '../hooks/useDeptEvents';
import { useProject } from '../hooks/useProject';
import { useChat } from '../hooks/useChat';
import { useDocumentTabs } from '../hooks/useDocumentTabs';
import { useDemoFlow } from '../hooks/useDemoFlow';
import { usePendingApprovals } from '../hooks/usePendingApprovals';
import { useProjectPicker } from '../hooks/useProjectPicker';
import { useDashboardUI } from '../hooks/useDashboardUI';
import DashboardLayout from '../components/DashboardLayout';
import AgentStreamPanel from '../components/AgentStreamPanel';
import ArtifactPanel from '../components/ArtifactPanel';
import ApprovalBanner from '../components/ApprovalBanner';
import ProjectPicker from '../components/ProjectPicker';
import SettingsMenu from '../components/SettingsMenu';
import SettingsPage from './SettingsPage';
import HelpDrawer from '../components/HelpDrawer';
import DemoTour from '../components/DemoTour';
import WorkflowGraphView from '../components/WorkflowGraph';

import { Button } from '../components/ui/Button';
import { docIdToPath } from '../utils/docPath';
import { approveDocumentAndResume } from '../utils/approveDocument';
import { cancelProcessing, onProjectUpdate } from '../api';
import type { Project } from '../types';
import { isProjectOnboardingDone } from '../utils/uiPrefs';
import ProjectOnboarding from '../components/ProjectOnboarding';
import { GlossaryTerm } from '../components/GlossaryTerm';

const STORAGE_KEY = 'shuji_chat';

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
  const { activeDepts } = useDeptEvents();
  const activeDeptsArr = Array.from(activeDepts);

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

  const { pendingApprovals, gateContext } = usePendingApprovals(project);
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

  // Centralized UI state (activity, uiMode, experienceLevel, panels, keyboard shortcuts)
  const {
    activity,
    onActivity: handleActivity,
    experienceLevel,
    onExperienceLevelChange: handleExperienceLevelChange,
    beginnerMode,
    artifactOpen,
    setArtifactOpen,
    settingsOpen,
    setSettingsOpen,
    showProjectOnboarding,
    setShowProjectOnboarding,
    openArtifact,
  } = useDashboardUI(hasTabs, pendingApprovals, openTab, docIdToPath);

  // When a document is selected from the sidebar (file tree), auto-open the artifact panel
  const handleDocSelectAndOpenPanel = useCallback(
    (path: string) => {
      handleDocSelect(path);
      setArtifactOpen(true);
    },
    [handleDocSelect, setArtifactOpen]
  );

  useEffect(() => {
    try {
      sessionStorage.setItem(STORAGE_KEY, JSON.stringify({ msgs: messages, discuss: discussMsgs }));
    } catch {}
  }, [messages, discussMsgs]);

  useEffect(() => {
    const unlisten = onProjectUpdate((payload: Project) => {
      setProject(payload);
    });
    return () => {
      unlisten.then((f) => f());
    };
  }, [setProject]);

  useEffect(() => {
    if (!project || showDemoTour) return;
    if (isProjectOnboardingDone()) return;
    setShowProjectOnboarding(true);
  }, [project?.id, showDemoTour]);

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

  const handleApproveDoc = useCallback(async (docId: string, comment?: string) => {
    await approveDocumentAndResume(docId, comment);
  }, []);

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
        onDocSelect={handleDocSelectAndOpenPanel}
        onShowDiff={(path) => openTab(path, 'diff')}
        pendingApprovalsCount={pendingApprovals.length}
        beginnerMode={beginnerMode}
        approvalBanner={
          gateContext.active ? (
            <ApprovalBanner
              context={gateContext}
              onView={() => openArtifact(docIdToPath(gateContext.docId!))}
              onApprove={(comment) => handleApproveDoc(gateContext.docId!, comment)}
              onStop={() =>
                cancelProcessing().catch(swallowError('ProjectDashboard.cancelProcessing'))
              }
            />
          ) : undefined
        }
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
            <button
              onClick={() => setArtifactOpen((v) => !v)}
              className={`text-xs px-2 py-1 rounded transition-colors ${
                artifactOpen
                  ? 'bg-gold/20 text-gold'
                  : 'text-ink-400 hover:text-ink-200 hover:bg-ink-800'
              }`}
              title={`${artifactOpen ? '关闭' : '打开'}架阁 (Ctrl+\)`}
            >
              <GlossaryTerm term="artifact">架阁</GlossaryTerm>
            </button>
            <HelpDrawer
              experienceLevel={experienceLevel}
              onExperienceLevelChange={handleExperienceLevelChange}
              onReplayOnboarding={() => setShowProjectOnboarding(true)}
            />
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
              onOpenGraph={() => handleActivity('graph')}
              endRef={chatEndRef}
              beginnerMode={beginnerMode}
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
              gateContext={gateContext}
              onApproveDoc={handleApproveDoc}
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
      {showProjectOnboarding && !showDemoTour && (
        <ProjectOnboarding onClose={() => setShowProjectOnboarding(false)} />
      )}
      {settingsOpen && (
        <div className="fixed inset-0 z-50 bg-surface-paper overflow-y-auto">
          <SettingsPage onClose={() => setSettingsOpen(false)} />
        </div>
      )}
    </>
  );
}
