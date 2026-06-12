import { Tabs } from './ui/Tabs';
import WorkflowRibbon from './WorkflowRibbon';
import WorkflowStatus from './WorkflowTimeline';
import DeptActivityFeed from './DeptActivityFeed';
import ChatPanel from './ChatPanel';
import AgentIdleState from './AgentIdleState';
import { docIdToPath } from '../utils/docPath';
import type { Project, ChatMessage, PlanInfo, PhaseRuntime } from '../types';
import type { Tab } from '../hooks/useChat';

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
  endRef: React.RefObject<HTMLDivElement | null>;
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
  endRef,
}: AgentStreamPanelProps) {
  const totalStageCount = phaseCount || phases.length;
  const completedStageCount = phases.filter(
    (p) => p.execution === 'Completed' || p.execution === 'MinorIssue'
  ).length;
  const isIdle = project && messages.length === 0 && activeDeptsCount === 0;

  return (
    <div className="flex-1 flex flex-col min-h-0 min-w-0">
      <WorkflowRibbon
        totalStageCount={totalStageCount}
        completedStageCount={completedStageCount}
        pendingCount={pendingApprovals.length}
        onPendingClick={() => {
          if (pendingApprovals.length > 0) onSelectDoc(docIdToPath(pendingApprovals[0]));
        }}
      />

      {project && (
        <WorkflowStatus
          phaseCount={phaseCount}
          phases={phases}
          overall={overall}
          activeDepts={activeDepts}
          planInfo={planInfo}
          pendingApprovals={pendingApprovals}
          onSelectDoc={onSelectDoc}
          collapsible
          defaultCollapsed
        />
      )}

      {project ? (
        <div className="flex-1 flex min-h-0">
          <div className="w-72 shrink-0 overflow-y-auto border-r border-fold bg-surface-paper/50">
            <DeptActivityFeed onDocClick={onSelectDoc} />
          </div>
          <div className="flex-1 flex flex-col min-w-0">
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
            {isIdle && tab === 'decision' ? (
              <>
                <AgentIdleState
                  project={project}
                  onDocSelect={onSelectDoc}
                  onOpenProject={onOpenProject}
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
                  onDiscuss={onDiscuss}
                  onCancelDiscuss={onCancelDiscuss}
                  onConvertToCommand={onConvertToCommand}
                  endRef={endRef}
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
                onDiscuss={onDiscuss}
                onCancelDiscuss={onCancelDiscuss}
                onConvertToCommand={onConvertToCommand}
                endRef={endRef}
              />
            )}
          </div>
        </div>
      ) : (
        <div className="flex-1 flex items-center justify-center text-body text-ink-400">
          请先开卷
        </div>
      )}
    </div>
  );
}
