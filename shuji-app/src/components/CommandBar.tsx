import { useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { getRoundMetrics } from '../api';
import { getDeptMeta, getDeptDisplayLabel } from '../constants';
import { useDeptEvents } from '../hooks/useDeptEvents';
import { useWorkflowTimeline } from '../hooks/useWorkflowTimeline';
import { docIdToPath } from '../utils/docPath';
import WorkflowTimeline from './WorkflowTimeline';
import PlanPanel from './PlanPanel';
import { ValidationSummary } from './ValidationSummary';
import type {
  RoundMetrics,
  PlanInfo,
  PhaseRuntime,
  PhaseExecutionStatus,
  TimelineNode,
  ValidationReport,
} from '../types';

export interface CommandBarProps {
  totalStageCount: number;
  completedStageCount: number;
  phaseCount: number;
  phases: PhaseRuntime[];
  overall: string;
  activeDepts: string[];
  planInfo: PlanInfo | null;
  pendingApprovals: string[];
  validationReport?: ValidationReport | null;
  validationLoading?: boolean;
  onSelectDoc: (docPath: string) => void;
  onSelectDept?: (dept: string) => void;
  onPendingClick?: () => void;
  onOpenGraph?: () => void;
}

const STATUS_ICONS: Record<string, string> = {
  NotStarted: '○',
  Designing: '●',
  Reviewing: '●',
  PendingApproval: '⚑',
  Rejected: '✗',
  Approved: '✓',
  TaskBreakdown: '●',
  Testing: '●',
  Implementing: '●',
  Checking: '●',
  Standards: '●',
  Completed: '✓',
  Logging: '●',
  MinorIssue: '●',
};

function statusTKey(status: string): string {
  const map: Record<string, string> = {
    NotStarted: 'workflow.notStarted',
    Designing: 'workflowStatus.designing',
    Reviewing: 'workflowStatus.reviewing',
    PendingApproval: 'workflow.pendingApproval',
    Rejected: 'workflow.rejected',
    Approved: 'workflow.approved',
    TaskBreakdown: 'workflowStatus.taskBreakdown',
    Testing: 'workflowStatus.testing',
    Implementing: 'workflowStatus.implementing',
    Checking: 'workflowStatus.checking',
    Standards: 'workflowStatus.standards',
    Completed: 'workflow.complete',
    Logging: 'workflowStatus.logging',
    MinorIssue: 'workflowStatus.minorIssue',
    Blocked: 'workflow.blocked',
  };
  return map[status] || status;
}

function statusColor(status: string): string {
  if (['Approved', 'Completed'].includes(status)) return 'text-jade';
  if (status === 'NotStarted') return 'text-ink-300';
  if (['PendingApproval', 'Rejected'].includes(status)) return 'text-vermillion';
  return 'text-gold';
}

function isBlocked(status: string): boolean {
  return status === 'PendingApproval' || status === 'Rejected';
}

const STORAGE_UI_KEY = 'shuji_ui_prefs';

function loadWorkflowExpanded(): boolean {
  try {
    const raw = localStorage.getItem(STORAGE_UI_KEY);
    if (raw) {
      const prefs = JSON.parse(raw) as { workflowCollapsed?: boolean };
      if (typeof prefs.workflowCollapsed === 'boolean') return !prefs.workflowCollapsed;
    }
  } catch {}
  return false;
}

function saveWorkflowCollapsed(collapsed: boolean) {
  try {
    const raw = localStorage.getItem(STORAGE_UI_KEY);
    const prefs = raw ? JSON.parse(raw) : {};
    prefs.workflowCollapsed = collapsed;
    localStorage.setItem(STORAGE_UI_KEY, JSON.stringify(prefs));
  } catch {}
}

export default function CommandBar({
  phaseCount,
  phases,
  overall,
  activeDepts,
  planInfo,
  pendingApprovals,
  validationReport = null,
  validationLoading = false,
  onSelectDoc,
  onSelectDept,
  onPendingClick,
  onOpenGraph,
}: CommandBarProps) {
  const { t, i18n } = useTranslation();
  const lang = i18n.language?.startsWith('en') ? 'en' : 'zh';
  const { latestLogs } = useDeptEvents();
  const {
    wfState,
    timelineNodes,
    recentDocIds,
    nextAction,
    pipelineProgress,
    gongbuBatch,
    hasFlowActivity,
    pipeline,
  } = useWorkflowTimeline({
    activeDepts,
    latestLogs,
    pendingApprovals,
    planInfo,
  });

  const [expanded, setExpanded] = useState(() => loadWorkflowExpanded());
  const [roundMetrics, setRoundMetrics] = useState<RoundMetrics | null>(null);
  const [elapsed, setElapsed] = useState('');

  useEffect(() => {
    if (hasFlowActivity && pendingApprovals.length > 0 && !loadWorkflowExpanded()) {
      setExpanded(true);
    }
  }, [hasFlowActivity, pendingApprovals.length]);

  useEffect(() => {
    if (pipeline && !loadWorkflowExpanded()) {
      setExpanded(true);
    }
  }, [pipeline]);

  const profileLabels: Record<string, string> = {
    greenfield_standard: t('workflow.newFeature'),
    brownfield_optimize: t('workflow.existingOptimization'),
    bugfix: t('workflow.bugfix'),
    demo: t('workflow.quickPrototype'),
  };

  function profileLabel(id: string): string {
    return profileLabels[id] || id;
  }

  const stageLabels: Record<string, string> = {
    init: t('workflow.initialize'),
    expand: t('workflow.expandRequirements'),
    design: t('workflow.design'),
    analysis: t('workflow.codeAnalysis'),
    plan: t('workflow.planning'),
    review: t('workflow.review'),
    approval: t('workflow.approval'),
    execution: t('workflow.execution'),
    summary: t('workflow.summary'),
    done: t('workflow.complete'),
  };

  function stageLabel(id: string): string {
    return stageLabels[id] || id;
  }

  useEffect(() => {
    const load = () => {
      getRoundMetrics()
        .then((m) => setRoundMetrics(m))
        .catch(() => {});
    };
    load();
    const timer = window.setInterval(load, 3000);
    return () => window.clearInterval(timer);
  }, []);

  useEffect(() => {
    if (!roundMetrics || roundMetrics.started_at <= 0) {
      setElapsed('');
      return;
    }
    const tick = () => {
      const secs = Math.floor((Date.now() - roundMetrics.started_at) / 1000);
      if (secs < 60) setElapsed(`${secs}s`);
      else if (secs < 3600) setElapsed(`${Math.floor(secs / 60)}m${secs % 60}s`);
      else setElapsed(`${Math.floor(secs / 3600)}h${Math.floor((secs % 3600) / 60)}m`);
    };
    tick();
    const timer = window.setInterval(tick, 1000);
    return () => clearInterval(timer);
  }, [roundMetrics]);

  const total = (phaseCount || phases.length) * 2 + 1;
  let done = 0;
  if (overall === 'Approved') done += 1;
  for (const phase of phases) {
    if (phase.design === 'Approved') done += 1;
    if (phase.execution === 'Completed') done += 1;
  }
  const progress =
    pipelineProgress && pipelineProgress.total > 0
      ? Math.round((pipelineProgress.done / pipelineProgress.total) * 100)
      : total > 0
        ? Math.round((done / total) * 100)
        : 0;

  const mainActiveDept = activeDepts.length > 0 ? activeDepts[activeDepts.length - 1] : null;
  const activeMeta = mainActiveDept ? getDeptMeta(mainActiveDept) : null;

  const isEmpty = phases.length === 0 && !hasFlowActivity && !roundMetrics;

  const handleTimelineNodeClick = (node: TimelineNode) => {
    if (node.docId) {
      onSelectDoc(docIdToPath(node.docId));
      return;
    }
    if (node.dept && onSelectDept) {
      onSelectDept(node.dept);
    }
  };

  if (isEmpty) {
    return (
      <div className="h-10 flex items-center px-4 border-b border-fold bg-surface-paper command-bar-glow">
        <span className="text-caption text-ink-500 font-display">{t('commandBar.idle')}</span>
      </div>
    );
  }

  return (
    <div className="shrink-0 bg-surface-elevated border-b border-fold command-bar-glow">
      <div className="h-10 flex items-center gap-3 px-4 min-w-0">
        {wfState && (
          <span className="text-caption px-1.5 py-[1px] rounded-full border border-ink-300 text-ink-500 bg-ink-100/30 whitespace-nowrap shrink-0">
            {profileLabel(wfState.profile_id)}
          </span>
        )}

        {wfState && wfState.current_stage !== 'init' && (
          <>
            <span className="text-caption text-ink-300 shrink-0">·</span>
            <span className="text-caption px-1.5 py-[1px] rounded-full border border-gold/30 text-gold-700 bg-gold/8 whitespace-nowrap shrink-0">
              {stageLabel(wfState.current_stage)}
            </span>
          </>
        )}

        {pipelineProgress && (
          <span
            className="text-caption text-ink-500 truncate max-w-[120px] shrink-0 hidden sm:inline"
            title={pipelineProgress.summary}
          >
            {pipelineProgress.done}/{pipelineProgress.total}
          </span>
        )}

        {progress > 0 && (
          <div className="flex items-center gap-2 max-w-40 shrink-0">
            <div className="flex-1 h-[6px] bg-ink-200 rounded-full overflow-hidden min-w-[64px]">
              <div
                className="h-full bg-gold rounded-full transition-all duration-500"
                style={{ width: `${Math.min(progress, 100)}%` }}
              />
            </div>
            <span className="text-caption text-ink-500 font-mono tabular-nums">{progress}%</span>
          </div>
        )}

        {activeMeta && (
          <span className="flex items-center gap-1.5 shrink-0">
            <span className="w-1.5 h-1.5 rounded-full bg-gold animate-pulse shrink-0" />
            <span className="text-ui font-display text-ink-700">
              {getDeptDisplayLabel(activeMeta, lang)}
            </span>
          </span>
        )}

        {nextAction && (
          <span
            className={`text-caption truncate min-w-0 ${
              nextAction.type === 'approval' ? 'text-vermillion font-medium' : 'text-ink-600'
            }`}
            title={nextAction.label}
          >
            {nextAction.label}
          </span>
        )}

        {elapsed && <span className="text-caption text-ink-500 font-mono shrink-0">{elapsed}</span>}

        <div className="flex-1 min-w-0" />

        {pendingApprovals.length > 0 && (
          <button
            onClick={onPendingClick ?? (() => onSelectDoc(docIdToPath(pendingApprovals[0])))}
            className="flex items-center gap-1 px-2 py-0.5 rounded text-caption border border-vermillion/30 text-vermillion bg-vermillion/8 hover:bg-vermillion/15 transition-colors shrink-0 stage-badge-pop"
          >
            <svg className="w-3 h-3" viewBox="0 0 24 24" fill="currentColor">
              <path d="M12 2L15.09 8.26L22 9.27L17 14.14L18.18 21.02L12 17.77L5.82 21.02L7 14.14L2 9.27L8.91 8.26L12 2Z" />
            </svg>
            {t('document.pendingApproval')} {pendingApprovals.length}
          </button>
        )}

        {onOpenGraph && (
          <button
            type="button"
            onClick={onOpenGraph}
            className="text-caption text-ink-400 hover:text-ink-600 shrink-0 hidden md:inline"
          >
            {t('activityBar.graph')}
          </button>
        )}

        {(phases.length > 0 || hasFlowActivity || pipeline) && (
          <button
            onClick={() => {
              setExpanded((prev) => {
                saveWorkflowCollapsed(!prev);
                return !prev;
              });
            }}
            className="text-caption text-ink-400 hover:text-ink-600 shrink-0"
          >
            {expanded ? t('commandBar.collapse') : t('commandBar.details')}
          </button>
        )}
      </div>

      {expanded && (
        <div
          className="px-4 pb-3 border-t border-fold/50 text-caption text-ink-600 overflow-y-auto space-y-2"
          style={{ maxHeight: 'var(--cockpit-command-expanded-max)' }}
        >
          {pipeline && <PlanPanel runtime={pipeline} defaultExpanded />}

          {(validationReport || validationLoading) && (
            <div>
              <span className="text-ink-500 font-medium block mb-1">
                {t('validation.latestReport')}
              </span>
              <ValidationSummary report={validationReport} loading={validationLoading} />
            </div>
          )}

          <div>
            <div className="flex items-center justify-between gap-2 pt-1.5 pb-1">
              <span className="text-ink-500 font-medium">{t('audit.timeline')}</span>
              {gongbuBatch && (
                <span className="text-ink-400">
                  {t('inspector.gongbuBatch')} {gongbuBatch.done}/{gongbuBatch.total}
                </span>
              )}
            </div>
            <WorkflowTimeline nodes={timelineNodes} onNodeClick={handleTimelineNodeClick} />
          </div>

          {recentDocIds.length > 0 && (
            <div className="pt-1 border-t border-fold/30">
              <span className="text-ink-500 font-medium block mb-1">
                {t('timeline.recentDocs')}
              </span>
              <div className="flex flex-wrap gap-1.5">
                {recentDocIds.map((docId) => (
                  <button
                    key={docId}
                    type="button"
                    onClick={() => onSelectDoc(docIdToPath(docId))}
                    className="px-2 py-0.5 rounded border border-ink-200 bg-ink-100/40 hover:bg-ink-100 font-mono text-ink-700"
                  >
                    {docId}
                    {pendingApprovals.includes(docId) && (
                      <span className="ml-1 text-vermillion">★</span>
                    )}
                  </button>
                ))}
              </div>
            </div>
          )}

          {wfState && (
            <div className="pt-1 border-t border-fold/30">
              <div className="flex items-center gap-2 mb-1 text-ink-500 font-medium">
                <span>{t('commandBar.workflow')}</span>
                <span className="text-caption px-1 rounded bg-ink-100/50">
                  {wfState.governance}
                </span>
              </div>
              <div className="flex items-center gap-1.5 flex-wrap">
                <span className="px-2 py-0.5 rounded-full border border-ink-200 text-ink-500">
                  {profileLabel(wfState.profile_id)}
                </span>
                <span className="text-ink-300">→</span>
                <span className="px-2 py-0.5 rounded-full border border-gold/30 text-gold-700 bg-gold/8">
                  {stageLabel(wfState.current_stage)}
                </span>
                <span className="text-ink-300 mx-1 text-caption">
                  ({wfState.execution_chain_id})
                </span>
              </div>
            </div>
          )}

          {phases.length > 0 && (
            <div className="space-y-0.5 pt-1 border-t border-fold/30">
              {phases.map((phase) => {
                const dStatus = typeof phase.design === 'string' ? phase.design : '';
                const eObj = phase.execution as PhaseExecutionStatus;
                const eIsBlocked = typeof eObj === 'object' && eObj !== null && 'Blocked' in eObj;
                const eStr = eIsBlocked
                  ? 'Blocked'
                  : typeof eObj === 'object' && eObj !== null
                    ? JSON.stringify(eObj)
                    : String(eObj);
                return (
                  <div key={phase.index} className="flex items-center gap-2">
                    <span className="font-mono text-ink-400 w-14 shrink-0">
                      {t('commandBar.phase')}
                      {phase.index}
                    </span>
                    <span
                      className={`${statusColor(dStatus)} ${isBlocked(dStatus) ? 'font-medium' : ''}`}
                    >
                      {STATUS_ICONS[dStatus] || '●'} {t(statusTKey(dStatus), dStatus)}
                    </span>
                    <span className="text-ink-300 mx-0.5">|</span>
                    <span
                      className={eIsBlocked ? 'text-vermillion font-medium' : statusColor(eStr)}
                    >
                      {eIsBlocked
                        ? `${STATUS_ICONS['Blocked'] || '⚑'} ${t('workflow.blocked')}`
                        : `${STATUS_ICONS[eStr] || '●'} ${t(statusTKey(eStr), eStr)}`}
                    </span>
                  </div>
                );
              })}
            </div>
          )}
        </div>
      )}
    </div>
  );
}
