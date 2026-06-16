import { useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { getRoundMetrics, getWorkflowState } from '../api';
import { getDeptMeta } from '../constants';
import { docIdToPath } from '../utils/docPath';
import type { RoundMetrics, PlanInfo, PhaseRuntime, PhaseExecutionStatus, WorkflowState as WFState } from '../types';

export interface CommandBarProps {
  totalStageCount: number;
  completedStageCount: number;
  phaseCount: number;
  phases: PhaseRuntime[];
  overall: string;
  activeDepts: string[];
  planInfo: PlanInfo | null;
  pendingApprovals: string[];
  onSelectDoc: (docPath: string) => void;
  onPendingClick?: () => void;
  onOpenGraph?: () => void;
}

// Icon mapping by English status code
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

function statusIcon(status: string): string {
  return STATUS_ICONS[status] || '●';
}

// Translation key lookup for status codes
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
  pendingApprovals,
  onSelectDoc,
  onPendingClick,
}: CommandBarProps) {
  const { t, i18n } = useTranslation();
  const lang = i18n.language?.startsWith('en') ? 'en' : 'zh';
  const [expanded, setExpanded] = useState(() => loadWorkflowExpanded());
  const [roundMetrics, setRoundMetrics] = useState<RoundMetrics | null>(null);
  const [elapsed, setElapsed] = useState('');
  const [wfState, setWfState] = useState<WFState | null>(null);

  // Profile label mapping
  const profileLabels: Record<string, string> = {
    greenfield_standard: t('workflow.newFeature'),
    brownfield_optimize: t('workflow.existingOptimization'),
    bugfix: t('workflow.bugfix'),
    demo: t('workflow.quickPrototype'),
  };

  function profileLabel(id: string): string {
    return profileLabels[id] || id;
  }

  // Stage label mapping
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

  useEffect(() => {
    const fetch = () => {
      getWorkflowState()
        .then(setWfState)
        .catch(() => setWfState(null));
    };
    fetch();
    const timer = setInterval(fetch, 3000);
    return () => clearInterval(timer);
  }, []);

  const total = (phaseCount || phases.length) * 2 + 1;
  let done = 0;
  if (overall === 'Approved') done += 1;
  for (const phase of phases) {
    if (phase.design === 'Approved') done += 1;
    if (phase.execution === 'Completed') done += 1;
  }
  const progress = total > 0 ? Math.round((done / total) * 100) : 0;

  const mainActiveDept = activeDepts.length > 0 ? activeDepts[activeDepts.length - 1] : null;
  const activeMeta = mainActiveDept ? getDeptMeta(mainActiveDept) : null;

  const isEmpty = phases.length === 0 && !wfState && !roundMetrics;

  if (isEmpty) {
    return (
      <div className="h-10 flex items-center px-4 border-b border-fold bg-surface-paper command-bar-glow">
        <span className="text-caption text-ink-500 font-display">{t('commandBar.idle')}</span>
      </div>
    );
  }

  return (
    <div className="shrink-0 bg-surface-elevated border-b border-fold command-bar-glow">
      <div className="h-10 flex items-center gap-3 px-4">
        {/* Workflow profile badge */}
        {wfState && (
          <span className="text-caption px-1.5 py-[1px] rounded-full border border-ink-300 text-ink-500 bg-ink-100/30 whitespace-nowrap">
            {profileLabel(wfState.profile_id)}
          </span>
        )}

        {/* Separator */}
        {wfState && wfState.current_stage !== 'init' && (
          <span className="text-caption text-ink-300">·</span>
        )}

        {/* Current stage badge */}
        {wfState && wfState.current_stage !== 'init' && (
          <span className="text-caption px-1.5 py-[1px] rounded-full border border-gold/30 text-gold-700 bg-gold/8 whitespace-nowrap">
            {stageLabel(wfState.current_stage)}
          </span>
        )}

        {/* Progress bar */}
        {progress > 0 && (
          <div className="flex items-center gap-2 max-w-40">
            <div className="flex-1 h-[6px] bg-ink-200 rounded-full overflow-hidden min-w-[80px]">
              <div
                className="h-full bg-gold rounded-full transition-all duration-500"
                style={{ width: `${Math.min(progress, 100)}%` }}
              />
            </div>
            <span className="text-caption text-ink-500 font-mono tabular-nums shrink-0">
              {progress}%
            </span>
          </div>
        )}

        {/* Active department */}
        {activeMeta && (
          <span className="flex items-center gap-1.5 shrink-0">
            <span className="w-1.5 h-1.5 rounded-full bg-gold animate-pulse shrink-0" />
            <span className="text-ui font-display text-ink-700">
              {lang === 'en' ? activeMeta.shortLabelEn : activeMeta.shortLabel}
            </span>
          </span>
        )}

        {/* Elapsed */}
        {elapsed && (
          <span className="text-caption text-ink-500 font-mono shrink-0">{elapsed}</span>
        )}

        {/* Spacer */}
        <div className="flex-1 min-w-0" />

        {/* 朱批 badge */}
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

        {/* Expand toggle */}
        {(phases.length > 0 || wfState) && (
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

      {/* Expanded details */}
      {expanded && (
        <div
          className="px-4 pb-2 border-t border-fold/50 text-caption text-ink-600 overflow-y-auto"
          style={{ maxHeight: 'var(--cockpit-command-expanded-max)' }}
        >
          {wfState && (
            <div className="pt-1.5 pb-1">
              <div className="flex items-center gap-2 mb-1 text-ink-500 font-medium">
                <span>{t('commandBar.workflow')}</span>
                <span className="text-caption px-1 rounded bg-ink-100/50">{wfState.governance}</span>
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
            <div className="space-y-0.5 pt-1.5 border-t border-fold/30">
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
                    <span className="font-mono text-ink-400 w-14 shrink-0">{t('commandBar.phase')}{phase.index}</span>
                    <span className={`${statusColor(dStatus)} ${isBlocked(dStatus) ? 'font-medium' : ''}`}>
                      {statusIcon(dStatus)} {t(statusTKey(dStatus), dStatus)}
                    </span>
                    <span className="text-ink-300 mx-0.5">|</span>
                    <span className={eIsBlocked ? 'text-vermillion font-medium' : statusColor(eStr)}>
                      {eIsBlocked ? `${statusIcon('Blocked')} ${t('workflow.blocked')}` : `${statusIcon(eStr)} ${t(statusTKey(eStr), eStr)}`}
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
