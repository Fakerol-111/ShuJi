import { useEffect, useCallback } from 'react';
import { classifyError, swallowError } from '../utils/error';
import { useDeptEvents } from '../hooks/useDeptEvents';
import { useChat } from '../hooks/useChat';
import { useDocumentTabs } from '../hooks/useDocumentTabs';
import { useDemoFlow } from '../hooks/useDemoFlow';
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
import { cancelProcessing } from '../api';
import { isProjectOnboardingDone } from '../utils/uiPrefs';
import ProjectOnboarding from '../components/ProjectOnboarding';
import { GlossaryTerm } from '../components/GlossaryTerm';
import { ProjectProvider, useProjectContext } from '../runtime/ProjectContext';
import { ApprovalProvider, useApprovalContext } from '../runtime/ApprovalContext';

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
  return (
    <ProjectProvider>
      <ApprovalProvider>
        <DashboardContent />
      </ApprovalProvider>
    </ProjectProvider>
  );
}

function DashboardContent() {
  const session = loadSession();
  const { activeDepts } = useDeptEvents();
  const activeDeptsArr = Array.from(activeDepts);

  const {
    project,
    recentDirs,
    setRecentDirs,
    error: projError,
    setError: setProjError,
    loadProjectIntoState,
  } = useProjectContext();

  const { pendingApprovals, gateContext, approvingDocId, approveDoc } = useApprovalContext();

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

  // Use global approveDoc from ApprovalContext — it has a lock + optimistic update
  const handleApproveDoc = useCallback(
    async (docId: string, comment?: string) => {
      await approveDoc(docId, comment);
    },
    [approveDoc]
  );

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
        approvalBanner={
          gateContext.active ? (
            <ApprovalBanner
              context={gateContext}
              onView={() => openArtifact(docIdToPath(gateContext.docId!))}
              onApprove={(comment) => handleApproveDoc(gateContext.docId!, comment)}
              onStop={() =>
                cancelProcessing().catch(swallowError('ProjectDashboard.cancelProcessing'))
              }
              resuming={!!approvingDocId}
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
