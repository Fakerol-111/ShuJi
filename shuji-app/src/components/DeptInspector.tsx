import { useRef, useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { getDeptMeta } from '../constants';
import { summarizeDeptStep } from '../utils/deptStepSummary';
import DeptActivityCard from './DeptActivityCard';
import RouteContextBar from './RouteContextBar';
import { SealLogo } from './SealLogo';
import { useDeptEvents } from '../hooks/useDeptEvents';
import type { DeptLogEntry, DeptStepEntry, PlanInfo } from '../types';

const ERROR_PREFIX = '❌';

interface DeptInspectorProps {
  dept: string | null;
  mode: 'single' | 'all';
  entries: DeptLogEntry[];
  active: boolean;
  onBack: () => void;
  onDocClick?: (path: string) => void;
  planInfo?: PlanInfo | null;
}

function DeptInspectorHeader({
  dept,
  active,
  entries,
  onBack,
}: {
  dept: string;
  active: boolean;
  entries: DeptLogEntry[];
  onBack: () => void;
}) {
  const { t } = useTranslation();
  const meta = getDeptMeta(dept);
  const color = meta?.color || '#8B7355';
  const latestEntry = entries.length > 0 ? entries[entries.length - 1] : null;
  const hasError = latestEntry?.action?.startsWith(ERROR_PREFIX) ?? false;

  return (
    <div className="shrink-0 border-b border-fold bg-surface-elevated px-4 py-3">
      <div className="flex items-center gap-3">
        <button
          onClick={onBack}
          className="text-ui text-ink-500 hover:text-vermillion transition-colors shrink-0 flex items-center gap-1"
        >
          <svg
            width="14"
            height="14"
            viewBox="0 0 16 16"
            fill="none"
            stroke="currentColor"
            strokeWidth="1.5"
          >
            <polyline points="10,3 5,8 10,13" />
          </svg>
          {t('inspector.backToDuty')}
        </button>
        <div className="flex items-center gap-2 min-w-0">
          <span
            className={`w-2.5 h-2.5 rounded-full shrink-0 ${active ? 'animate-pulse' : ''}`}
            style={{ backgroundColor: color }}
          />
          <span className="text-sm font-semibold text-ink-800 truncate" style={{ color }}>
            {meta?.label || dept}
          </span>
          <span
            className={`text-caption px-1.5 py-0.5 rounded font-medium ${
              hasError
                ? 'bg-vermillion-light text-vermillion'
                : active
                  ? 'text-jade bg-jade-light'
                  : 'text-ink-400 bg-ink-100'
            }`}
          >
            {hasError
              ? t('inspector.error')
              : active
                ? t('inspector.executing')
                : t('inspector.idle')}
          </span>
        </div>
      </div>
      {latestEntry && (
        <div className="text-caption text-ink-600 mt-1 ml-1 truncate">{latestEntry.action}</div>
      )}
    </div>
  );
}

function DeptInspectorEmpty() {
  const { t } = useTranslation();
  return (
    <div className="flex-1 flex items-center justify-center">
      <div className="text-center space-y-2">
        <div className="inline-flex">
          <SealLogo size={32} />
        </div>
        <div className="text-ui text-ink-400">{t('inspector.noReports')}</div>
      </div>
    </div>
  );
}

function DeptInspectorFeed({
  entries,
  onDocClick,
}: {
  entries: DeptLogEntry[];
  onDocClick?: (path: string) => void;
}) {
  const { t } = useTranslation();
  const feedRef = useRef<HTMLDivElement>(null);
  const [autoScroll, setAutoScroll] = useState(true);

  useEffect(() => {
    if (autoScroll && feedRef.current) {
      feedRef.current.scrollTop = feedRef.current.scrollHeight;
    }
  }, [entries, autoScroll]);

  const handleScroll = () => {
    if (!feedRef.current) return;
    const { scrollTop, scrollHeight, clientHeight } = feedRef.current;
    const atBottom = scrollHeight - scrollTop - clientHeight < 40;
    if (!atBottom && autoScroll) setAutoScroll(false);
    if (atBottom && !autoScroll) setAutoScroll(true);
  };

  if (entries.length === 0) return <DeptInspectorEmpty />;

  return (
    <div ref={feedRef} onScroll={handleScroll} className="flex-1 overflow-y-auto">
      {!autoScroll && (
        <div className="sticky top-0 z-10 px-3 py-1 text-center">
          <button
            onClick={() => setAutoScroll(true)}
            className="text-caption text-gold hover:text-gold-dark bg-surface-paper/80 rounded px-2 py-0.5"
          >
            {t('common.refresh')}
          </button>
        </div>
      )}
      <div className="divide-y divide-ink-100/50 px-2 py-1">
        {entries.map((entry, i) => (
          <DeptActivityCard
            key={`${entry.ts}-${entry.action}-${i}`}
            entry={entry}
            onDocClick={onDocClick}
          />
        ))}
      </div>
    </div>
  );
}

function PlanInfoCard({ info }: { info: PlanInfo }) {
  const { t } = useTranslation();
  return (
    <div
      className="shrink-0 mx-4 mt-3 bg-surface-parchment border border-fold rounded-xl px-3 py-2"
      style={{ borderLeft: '3px solid var(--dept-gongbu)' }}
    >
      <div className="font-display text-caption text-ink-600 font-semibold mb-1">
        {t('inspector.gongbuBatch')}
      </div>
      <div className="space-y-0.5">
        {info.batches.map((b, i) => (
          <div key={i} className="flex items-center gap-1.5 text-caption font-mono">
            <span
              className={`w-1.5 h-1.5 rounded-full shrink-0 ${b.status === 'done' ? 'bg-jade' : b.status === 'current' ? 'bg-gold animate-pulse' : 'bg-ink-300'}`}
            />
            <span
              className={
                b.status === 'done'
                  ? 'text-ink-400 line-through'
                  : b.status === 'current'
                    ? 'text-ink-800 font-medium'
                    : 'text-ink-500'
              }
            >
              {b.name}
            </span>
            {b.status === 'current' && (
              <span className="text-ink-400 text-caption ml-auto truncate">{b.goal}</span>
            )}
          </div>
        ))}
      </div>
    </div>
  );
}

function StepCard({ entry, humanMode }: { entry: DeptStepEntry; humanMode: boolean }) {
  const { t, i18n } = useTranslation();
  const lang = i18n.language?.startsWith('en') ? 'en' : 'zh';
  const kind = entry.kind;

  if (humanMode) {
    const summary = summarizeDeptStep(entry, lang);
    if (!summary && kind.type !== 'text_delta' && kind.type !== 'reasoning_delta') return null;
    const display =
      summary ??
      (kind.type === 'text_delta'
        ? kind.delta
        : kind.type === 'reasoning_delta'
          ? `💭 ${kind.delta}`
          : '');
    if (!display) return null;
    const isError = kind.type === 'tool_result' && !kind.ok;
    return (
      <div
        className={`flex items-start gap-2 py-1.5 px-3 text-caption ${
          isError ? 'text-vermillion bg-vermillion/5' : 'text-ink-600'
        }`}
      >
        <span className="text-gold shrink-0 mt-0.5">▸</span>
        <span className="leading-snug">{display}</span>
      </div>
    );
  }

  switch (kind.type) {
    case 'thinking': {
      const [expanded, setExpanded] = useState(false);
      const text = kind.content;
      return (
        <div className="border-l-2 border-jade/40 pl-2 py-1">
          <button
            onClick={() => setExpanded(!expanded)}
            className="text-caption text-ink-500 hover:text-ink-700 font-mono flex items-center gap-1"
          >
            <span>{expanded ? '▾' : '▸'}</span>
            <span className="text-jade font-semibold">{t('inspector.thinking')}</span>
          </button>
          {expanded && (
            <div className="text-caption text-ink-600 mt-1 whitespace-pre-wrap font-mono leading-relaxed max-h-60 overflow-y-auto">
              {text}
            </div>
          )}
        </div>
      );
    }
    case 'tool_call': {
      const argsStr = kind.args
        ? Object.entries(kind.args)
            .slice(0, 2)
            .map(([k, v]) => `${k}=${JSON.stringify(v).slice(0, 40)}`)
            .join(', ')
        : '';
      return (
        <div className="flex items-start gap-2 py-0.5 px-2 rounded hover:bg-ink-100/20">
          <svg
            className="w-3.5 h-3.5 shrink-0 mt-0.5 text-ink-400"
            viewBox="0 0 16 16"
            fill="none"
            stroke="currentColor"
            strokeWidth="1.5"
          >
            <circle cx="8" cy="8" r="6" />
            <line x1="8" y1="4" x2="8" y2="8" />
            <line x1="8" y1="8" x2="11" y2="10" />
          </svg>
          <div className="min-w-0 flex-1">
            <code className="text-caption font-mono font-semibold text-ink-700">{kind.tool}</code>
            {argsStr && (
              <span className="text-caption text-ink-500 font-mono ml-1 truncate">{argsStr}</span>
            )}
          </div>
        </div>
      );
    }
    case 'tool_result': {
      return (
        <div className="flex items-start gap-2 py-0.5 px-2 rounded">
          <span
            className={`text-caption font-mono font-semibold shrink-0 mt-0.5 ${kind.ok ? 'text-jade' : 'text-vermillion'}`}
          >
            {kind.ok ? t('common.completed') : t('common.failed')}
          </span>
          <div className="min-w-0 flex-1">
            <code
              className={`text-caption font-mono font-semibold ${kind.ok ? 'text-jade' : 'text-vermillion'}`}
            >
              {kind.tool}
            </code>
            <span className="text-caption text-ink-500 font-mono ml-1">
              {kind.summary.slice(0, 120)}
            </span>
          </div>
        </div>
      );
    }
    case 'text': {
      return (
        <div className="text-caption text-ink-600 px-2 py-1 whitespace-pre-wrap font-mono leading-relaxed">
          {kind.content.slice(0, 300)}
        </div>
      );
    }
    default:
      return null;
  }
}

/** Resolve department steps — keys are normalized to CN labels in useDeptEvents. */
function resolveDeptSteps(deptSteps: Map<string, DeptStepEntry[]>, dept: string): DeptStepEntry[] {
  const meta = getDeptMeta(dept);
  if (!meta) return deptSteps.get(dept) || [];
  return deptSteps.get(meta.label) || [];
}

export default function DeptInspector({
  dept,
  mode,
  entries,
  active,
  onBack,
  onDocClick,
  planInfo,
}: DeptInspectorProps) {
  const { t } = useTranslation();
  const { deptSteps } = useDeptEvents();
  const [humanMode, setHumanMode] = useState(true);
  const isAllMode = mode === 'all';
  const showPlan =
    !isAllMode &&
    dept &&
    getDeptMeta(dept)?.key === 'gongbushangshu' &&
    planInfo &&
    planInfo.batches.length > 0;
  const steps = !isAllMode && dept ? resolveDeptSteps(deptSteps, dept) : [];

  return (
    <div className="flex-1 flex flex-col min-w-0">
      {!isAllMode && dept && (
        <DeptInspectorHeader dept={dept} active={active} entries={entries} onBack={onBack} />
      )}
      {isAllMode && (
        <div className="shrink-0 border-b border-fold bg-surface-elevated px-4 py-3 flex items-center gap-3">
          <button
            onClick={onBack}
            className="text-ui text-ink-500 hover:text-vermillion transition-colors shrink-0 flex items-center gap-1"
          >
            <svg
              width="14"
              height="14"
              viewBox="0 0 16 16"
              fill="none"
              stroke="currentColor"
              strokeWidth="1.5"
            >
              <polyline points="10,3 5,8 10,13" />
            </svg>
            {t('inspector.backToDuty')}
          </button>
          <span className="text-ui font-semibold text-ink-700">{t('inspector.deptActivity')}</span>
          <span className="text-caption text-ink-400">{entries.length} 条</span>
        </div>
      )}
      {!isAllMode && <RouteContextBar entries={entries} />}
      {showPlan && planInfo && <PlanInfoCard info={planInfo} />}
      {steps.length > 0 && (
        <div className="shrink-0 border-b border-fold">
          <div className="flex items-center justify-between px-3 py-1.5 bg-surface-elevated/50">
            <span className="text-caption text-ink-500 font-medium">
              {humanMode ? t('inspector.activitySummary') : t('inspector.technicalSteps')}
            </span>
            <button
              type="button"
              onClick={() => setHumanMode((v) => !v)}
              className="text-caption text-gold hover:text-gold-dark"
            >
              {humanMode ? t('inspector.showTechnical') : t('inspector.showSummary')}
            </button>
          </div>
          <div className="divide-y divide-ink-100/30 max-h-[40vh] overflow-y-auto">
            {steps.map((step, i) => (
              <StepCard key={`${step.ts}-${i}`} entry={step} humanMode={humanMode} />
            ))}
          </div>
        </div>
      )}
      <DeptInspectorFeed entries={entries} onDocClick={onDocClick} />
    </div>
  );
}
