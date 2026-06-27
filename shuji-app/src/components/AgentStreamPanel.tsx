import { useState, useEffect, useRef } from 'react';
import { useTranslation } from 'react-i18next';
import { Tabs } from './ui/Tabs';
import CommandBar from './CommandBar';
import DeptCardRail from './DeptCardRail';
import DeptInspector from './DeptInspector';
import ChatPanel from './ChatPanel';
import { useDeliveryValidation } from '../hooks/useDeliveryValidation';
import ProjectOverview from './ProjectOverview';
import { getDeptMeta } from '../constants';
import { useDeptEvents } from '../hooks/useDeptEvents';
import { deptMatches, isDeptActive } from '../utils/deptLog';
import type { Project, ChatMessage, PlanInfo } from '../types';
import type { Tab } from '../hooks/useChat';
import type { PhaseRuntime } from '../types';
import type { ChatInputHandle } from './ChatInput';

interface AgentStreamPanelProps {
  project: Project | null;
  tab: Tab;
  setTab: (tab: Tab) => void;
  messages: ChatMessage[];
  discussMsgs: ChatMessage[];
  discussing: boolean;
  planInfo: PlanInfo | null;
  activeDeptsCount: number;
  activeDepts: string[];
  pendingApprovals: string[];
  phases: PhaseRuntime[];
  phaseCount: number;
  overall: string;
  onOption: (key: string, supplement?: string) => void;
  onSend: (text: string) => void;
  onRetrySend: (text: string, ts: string) => Promise<void>;
  onDiscuss: (text: string) => void;
  onCancelDiscuss: () => void;
  onConvertToCommand: (text: string) => void;
  onSelectDoc: (path: string) => void;
  onOpenProject?: () => void;
  onFillInput?: (text: string) => void;
  onOpenGraph?: () => void;
  endRef: React.RefObject<HTMLDivElement | null>;
  beginnerMode?: boolean;
}

export default function AgentStreamPanel({
  project,
  tab,
  setTab,
  messages,
  discussMsgs,
  discussing,
  planInfo,
  activeDeptsCount,
  activeDepts,
  pendingApprovals,
  phases,
  phaseCount,
  overall,
  onOption,
  onSend,
  onRetrySend,
  onDiscuss,
  onCancelDiscuss,
  onConvertToCommand,
  onSelectDoc,
  onOpenProject,
  onOpenGraph,
  endRef,
  beginnerMode = false,
}: AgentStreamPanelProps) {
  const { t } = useTranslation();
  const { latestLogs, logEntries } = useDeptEvents();
  const [selectedDept, setSelectedDept] = useState<string | null>(null);
  const [pinDept, setPinDept] = useState(false);
  const showChat = selectedDept === null || selectedDept === '内阁';
  const inInspector = !showChat && selectedDept !== null;

  // Auto-follow: when not pinned and in inspector mode, switch to latest active dept
  // when activeDepts changes and the latest log's dept differs from current selection.
  useEffect(() => {
    if (pinDept) return;
    if (!inInspector) return;
    if (activeDepts.length === 0) return;
    const latestActive = activeDepts[activeDepts.length - 1];
    if (latestActive && selectedDept !== latestActive && selectedDept !== '__all__') {
      const meta = getDeptMeta(latestActive);
      if (meta && meta.label !== '内阁') {
        setSelectedDept(meta.label);
      }
    }
  }, [activeDepts, pinDept, inInspector, selectedDept]);

  const chatInputRef = useRef<ChatInputHandle>(null);
  const handleFillInput = (text: string) => chatInputRef.current?.setText(text);
  const { report: validationReport, loading: validationLoading } = useDeliveryValidation(
    project?.working_dir,
    activeDepts
  );
  const totalStageCount = phaseCount || phases.length;
  const completedStageCount = phases.filter(
    (p) => p.execution === 'Completed' || p.execution === 'MinorIssue'
  ).length;
  const isIdle = project && messages.length === 0 && activeDeptsCount === 0;

  return (
    <div className="flex-1 flex flex-col min-h-0 min-w-0">
      <CommandBar
        totalStageCount={totalStageCount}
        completedStageCount={completedStageCount}
        phaseCount={phaseCount}
        phases={phases}
        overall={overall}
        activeDepts={activeDepts}
        planInfo={planInfo}
        pendingApprovals={pendingApprovals}
        validationReport={validationReport}
        validationLoading={validationLoading}
        onSelectDoc={onSelectDoc}
        onSelectDept={(dept) => {
          const meta = getDeptMeta(dept);
          const label = meta?.label ?? dept;
          if (label === '内阁') {
            setSelectedDept(null);
          } else {
            setSelectedDept(label);
            setPinDept(true);
          }
        }}
        onOpenGraph={onOpenGraph}
      />

      {project ? (
        <div className="flex-1 flex min-h-0">
          {!beginnerMode && (
            <DeptCardRail
              selected={selectedDept}
              onSelect={setSelectedDept}
              activeDepts={activeDepts}
              latestLogs={latestLogs}
              planInfo={planInfo}
              pinDept={pinDept}
              onTogglePin={() => setPinDept((p) => !p)}
            />
          )}
          {showChat ? (
            <div className="flex-1 flex flex-col min-w-0 stage-edict">
              <div className="border-b border-fold bg-surface-elevated shrink-0 px-4 py-2 flex items-center justify-between">
                <span className="font-display text-ui font-semibold text-ink-800">
                  {t('inspector.backToDuty')}
                </span>
                <Tabs
                  tabs={[
                    { key: 'decision', label: t('inspector.decision') },
                    { key: 'discuss', label: t('inspector.discussion') },
                  ]}
                  activeKey={tab}
                  onChange={(k) => setTab(k as Tab)}
                />
              </div>
              {isIdle && tab === 'decision' ? (
                <>
                  <ProjectOverview
                    project={project}
                    activeDepts={activeDepts}
                    planInfo={planInfo}
                    onOpenProject={onOpenProject ?? (() => {})}
                    onDocSelect={onSelectDoc}
                    onFillInput={handleFillInput}
                    validationReport={validationReport}
                  />
                  <ChatPanel
                    tab={tab}
                    messages={messages}
                    discussMsgs={discussMsgs}
                    discussing={discussing}
                    planInfo={planInfo}
                    activeDeptsCount={activeDeptsCount}
                    onOption={onOption}
                    onSend={onSend}
                    onRetrySend={onRetrySend}
                    onDocumentClick={onSelectDoc}
                    onDiscuss={onDiscuss}
                    onCancelDiscuss={onCancelDiscuss}
                    onConvertToCommand={onConvertToCommand}
                    endRef={endRef}
                    chatInputRef={chatInputRef}
                  />
                </>
              ) : (
                <ChatPanel
                  tab={tab}
                  messages={messages}
                  discussMsgs={discussMsgs}
                  discussing={discussing}
                  planInfo={planInfo}
                  activeDeptsCount={activeDeptsCount}
                  onOption={onOption}
                  onSend={onSend}
                  onRetrySend={onRetrySend}
                  onDocumentClick={onSelectDoc}
                  onDiscuss={onDiscuss}
                  onCancelDiscuss={onCancelDiscuss}
                  onConvertToCommand={onConvertToCommand}
                  endRef={endRef}
                  chatInputRef={chatInputRef}
                />
              )}
            </div>
          ) : (
            <div
              className="flex-1 flex flex-col min-w-0 stage-inspect cockpit-fade-in"
              data-dept={selectedDept !== '__all__' ? getDeptMeta(selectedDept)?.key : undefined}
            >
              {selectedDept === '__all__' ? (
                <DeptInspector
                  dept={null}
                  mode="all"
                  entries={logEntries}
                  active={false}
                  onBack={() => setSelectedDept(null)}
                  onDocClick={onSelectDoc}
                />
              ) : (
                <DeptInspector
                  dept={selectedDept}
                  mode="single"
                  entries={logEntries.filter((e) => deptMatches(e.dept, selectedDept!))}
                  active={isDeptActive(selectedDept!, activeDepts)}
                  onBack={() => setSelectedDept(null)}
                  onDocClick={onSelectDoc}
                  planInfo={planInfo}
                />
              )}
            </div>
          )}
        </div>
      ) : (
        <div className="flex-1 flex items-center justify-center text-body text-ink-400">
          {t('inspector.pleaseOpen')}
        </div>
      )}
    </div>
  );
}
