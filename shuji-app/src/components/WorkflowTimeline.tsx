import { useState, useEffect } from 'react';
import { getWorkflowState } from '../api';
import { docIdToPath } from '../utils/docPath';
import type {
  PlanInfo,
  PhaseRuntime,
  PhaseExecutionStatus,
  WorkflowState as WFState,
} from '../types';

interface WorkflowStatusProps {
  phaseCount: number;
  phases: PhaseRuntime[];
  overall: string;
  activeDepts: string[];
  planInfo: PlanInfo | null;
  pendingApprovals: string[];
  onSelectDoc: (docPath: string) => void;
  collapsible?: boolean;
  defaultCollapsed?: boolean;
}

const STORAGE_UI_KEY = 'shuji_ui_prefs';

function loadWorkflowExpanded(defaultCollapsed?: boolean): boolean {
  try {
    const raw = localStorage.getItem(STORAGE_UI_KEY);
    if (raw) {
      const prefs = JSON.parse(raw) as { workflowCollapsed?: boolean };
      if (typeof prefs.workflowCollapsed === 'boolean') {
        return !prefs.workflowCollapsed;
      }
    }
  } catch {}
  return !defaultCollapsed;
}

function saveWorkflowCollapsed(collapsed: boolean) {
  try {
    const raw = localStorage.getItem(STORAGE_UI_KEY);
    const prefs = raw ? JSON.parse(raw) : {};
    prefs.workflowCollapsed = collapsed;
    localStorage.setItem(STORAGE_UI_KEY, JSON.stringify(prefs));
  } catch {}
}

// ── Profile display names ──────────────────────────────────────

const PROFILE_LABELS: Record<string, string> = {
  greenfield_standard: '新功能',
  brownfield_optimize: '存量优化',
  bugfix: '缺陷修复',
  demo: '快速原型',
};

const STAGE_LABELS: Record<string, string> = {
  init: '初始化',
  expand: '需求展开',
  design: '方案设计',
  analysis: '代码分析',
  plan: '方案规划',
  review: '审查',
  approval: '批复',
  execution: '执行',
  summary: '汇总',
  done: '完成',
};

function stageLabel(id: string): string {
  return STAGE_LABELS[id] || id;
}

function profileLabel(id: string): string {
  return PROFILE_LABELS[id] || id;
}

// ── Color tokens mapped to status ───────────────────────────

function statusDisplay(shortLabel: string): string {
  const icons: Record<string, string> = {
    未开始: '○',
    设计: '●',
    审查: '●',
    待批: '⚑',
    驳回: '✗',
    批准: '✓',
    拆解: '●',
    测试: '●',
    编码: '●',
    验证: '●',
    规范: '●',
    完成: '✓',
    记录: '●',
    问题: '●',
  };
  return `${icons[shortLabel] || '●'} ${shortLabel}`;
}

function statusColor(status: string): string {
  if (['Approved', 'Completed'].includes(status)) return 'text-jade';
  if (status === 'NotStarted') return 'text-ink-300';
  if (['PendingApproval', 'Rejected'].includes(status)) return 'text-vermillion';
  return 'text-gold';
}

function designShortLabel(status: string): string {
  const map: Record<string, string> = {
    NotStarted: '未开始',
    Designing: '设计',
    Reviewing: '审查',
    PendingApproval: '待批',
    Rejected: '驳回',
    Approved: '批准',
  };
  return map[status] || status;
}

function execShortLabel(status: string): string {
  const map: Record<string, string> = {
    NotStarted: '未开始',
    TaskBreakdown: '拆解',
    Testing: '测试',
    Implementing: '编码',
    Checking: '验证',
    Standards: '规范',
    Logging: '记录',
    MinorIssue: '问题',
    Completed: '完成',
  };
  return map[status] || status;
}

function isBlocked(status: string): boolean {
  return status === 'PendingApproval' || status === 'Rejected';
}

export default function WorkflowStatus({
  phaseCount,
  phases,
  overall,
  activeDepts,
  planInfo,
  pendingApprovals,
  onSelectDoc,
  collapsible,
  defaultCollapsed,
}: WorkflowStatusProps) {
  const [expanded, setExpanded] = useState(() => loadWorkflowExpanded(defaultCollapsed));
  const [wfState, setWfState] = useState<WFState | null>(null);

  // ── Poll workflow state every 3s (created after first send_message) ──
  useEffect(() => {
    const fetch = () => {
      getWorkflowState()
        .then(setWfState)
        .catch((e) => {
          console.error('加载工作流状态失败', e);
          setWfState(null);
        });
    };
    fetch();
    const timer = setInterval(fetch, 3000);
    return () => clearInterval(timer);
  }, []);

  // ── Calculate overall progress ──
  const total = (phaseCount || phases.length) * 2 + 1;
  let done = 0;
  if (overall === 'Approved') done += 1;
  for (const phase of phases) {
    if (phase.design === 'Approved') done += 1;
    if (phase.execution === 'Completed') done += 1;
  }
  const progress = total > 0 ? Math.round((done / total) * 100) : 0;

  // ── Build blocker badges ──
  const blockers: { type: string; text: string; onClick?: () => void }[] = [];

  if (pendingApprovals.length > 0) {
    for (const docId of pendingApprovals.slice(0, 2)) {
      blockers.push({
        type: '朱批',
        text: docId,
        onClick: () => onSelectDoc(docIdToPath(docId)),
      });
    }
  }

  if (activeDepts.length > 0) {
    blockers.push({
      type: '执行',
      text: activeDepts.join('、'),
    });
  }

  if (planInfo && !planInfo.complete && planInfo.batches.length > 0) {
    const ndone = planInfo.batches.filter((b) => b.status === 'done').length;
    blockers.push({
      type: '工部',
      text: `计划 ${ndone}/${planInfo.batches.length}`,
    });
  }

  if (phases.length === 0 && !wfState) {
    return (
      <div className="bg-surface-paper border-b border-fold shrink-0 px-4 py-1.5">
        <span className="text-caption text-ink-400">尚未启动流程</span>
      </div>
    );
  }

  const showToggle = (phases.length > 0 || wfState) && collapsible;

  return (
    <div className="bg-surface-paper border-b border-fold shrink-0">
      <div className="px-4 py-1.5 flex items-center gap-3 min-h-[36px]">
        {/* Progress bar */}
        <div className="flex items-center gap-2 flex-1 min-w-0">
          <div className="flex-1 h-2 bg-ink-200 rounded-full overflow-hidden max-w-44">
            <div
              className="h-full bg-gold transition-all duration-500 rounded-full"
              style={{ width: `${Math.min(progress, 100)}%` }}
            />
          </div>
          <span className="text-caption text-ink-500 font-mono tabular-nums w-[3ch]">
            {progress}%
          </span>
        </div>

        {/* Workflow profile badge */}
        {wfState && (
          <span className="text-caption px-1.5 py-[1px] rounded-full border border-ink-300 text-ink-500 bg-ink-100/30 whitespace-nowrap">
            {profileLabel(wfState.profile_id)}
          </span>
        )}

        {/* Current stage badge */}
        {wfState && wfState.current_stage !== 'init' && (
          <span className="text-caption px-1.5 py-[1px] rounded-full border border-gold/30 text-gold-700 bg-gold/8 whitespace-nowrap">
            {stageLabel(wfState.current_stage)}
          </span>
        )}

        {/* Blocker badges */}
        <div className="flex items-center gap-1.5 flex-wrap">
          {blockers.map((b, i) => (
            <button
              key={i}
              onClick={b.onClick}
              className={[
                'text-caption px-1.5 py-[1px] rounded-full border flex items-center gap-1 whitespace-nowrap',
                b.type === '朱批'
                  ? 'border-vermillion/30 text-vermillion bg-vermillion/8 hover:bg-vermillion/15'
                  : b.type === '执行'
                    ? 'border-gold/30 text-gold-700 bg-gold/8'
                    : 'border-ink-200 text-ink-500 bg-ink-100/50',
                b.onClick ? 'cursor-pointer' : 'cursor-default',
              ]
                .filter(Boolean)
                .join(' ')}
            >
              {b.type === '朱批' && (
                <svg className="w-3 h-3 shrink-0" viewBox="0 0 24 24" fill="currentColor">
                  <path d="M12 2L15.09 8.26L22 9.27L17 14.14L18.18 21.02L12 17.77L5.82 21.02L7 14.14L2 9.27L8.91 8.26L12 2Z" />
                </svg>
              )}
              {b.type === '执行' && (
                <span className="w-1.5 h-1.5 rounded-full bg-gold animate-pulse inline-block shrink-0" />
              )}
              {b.type === '工部' && (
                <svg
                  className="w-3 h-3 shrink-0"
                  viewBox="0 0 24 24"
                  fill="none"
                  stroke="currentColor"
                  strokeWidth={2}
                >
                  <path
                    strokeLinecap="round"
                    strokeLinejoin="round"
                    d="M9 5H7a2 2 0 00-2 2v12a2 2 0 002 2h10a2 2 0 002-2V7a2 2 0 00-2-2h-2M9 5a2 2 0 002 2h2a2 2 0 002-2M9 5a2 2 0 012-2h2a2 2 0 012 2"
                  />
                </svg>
              )}
              <span className="truncate max-w-24">{b.text}</span>
            </button>
          ))}

          {blockers.length === 0 &&
            (overall === 'Approved' && phases.every((p) => p.execution === 'Completed') ? (
              <span className="text-caption text-jade">所有阶段已完成</span>
            ) : (
              <span className="text-caption text-ink-400">流程运行中</span>
            ))}
        </div>

        {/* Expand toggle */}
        {showToggle && (
          <button
            onClick={() =>
              setExpanded((prev) => {
                const next = !prev;
                saveWorkflowCollapsed(!next);
                return next;
              })
            }
            className="text-caption text-ink-400 hover:text-ink-600 shrink-0"
          >
            {expanded ? '收起' : '详情'}
          </button>
        )}
      </div>

      {/* Expanded details */}
      {expanded && (
        <div className="px-4 pb-2 border-t border-fold/50 text-caption text-ink-600">
          {/* Workflow stage section */}
          {wfState && (
            <div className="pt-1.5 pb-1">
              <div className="flex items-center gap-2 mb-1 text-ink-500 font-medium">
                <span>工作流</span>
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

          {/* Phase details section */}
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
                    <span className="font-mono text-ink-400 w-14 shrink-0">阶段{phase.index}</span>
                    <span
                      className={`${statusColor(dStatus)} ${isBlocked(dStatus) ? 'font-medium' : ''}`}
                    >
                      {statusDisplay(designShortLabel(dStatus))}
                    </span>
                    <span className="text-ink-300 mx-0.5">|</span>
                    <span
                      className={eIsBlocked ? 'text-vermillion font-medium' : statusColor(eStr)}
                    >
                      {eIsBlocked ? `⚑ 阻塞` : statusDisplay(execShortLabel(eStr))}
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
