import { useTranslation } from 'react-i18next';
import { getDeptMeta } from '../constants';
import type { TimelineNode } from '../types';

export interface WorkflowTimelineProps {
  nodes: TimelineNode[];
  onNodeClick?: (node: TimelineNode) => void;
  compact?: boolean;
}

const STATUS_STYLES: Record<TimelineNode['status'], string> = {
  done: 'bg-jade/15 border-jade/40 text-jade',
  active: 'bg-gold/15 border-gold/50 text-gold-800 ring-1 ring-gold/30',
  pending: 'bg-ink-100/40 border-ink-200 text-ink-500',
  failed: 'bg-vermillion/10 border-vermillion/40 text-vermillion',
  waiting: 'bg-vermillion/10 border-vermillion/40 text-vermillion animate-pulse',
  retrying: 'bg-amber/10 border-amber/40 text-amber',
  cancelled: 'bg-ink-100/40 border-ink-300 text-ink-500 line-through',
  deadlocked: 'bg-vermillion/15 border-vermillion/60 text-vermillion',
};

const STATUS_DOT: Record<TimelineNode['status'], string> = {
  done: 'bg-jade',
  active: 'bg-gold animate-pulse',
  pending: 'bg-ink-300',
  failed: 'bg-vermillion',
  waiting: 'bg-vermillion animate-pulse',
  retrying: 'bg-amber animate-pulse',
  cancelled: 'bg-ink-400',
  deadlocked: 'bg-vermillion',
};

export default function WorkflowTimeline({
  nodes,
  onNodeClick,
  compact = false,
}: WorkflowTimelineProps) {
  const { t, i18n } = useTranslation();
  const lang = i18n.language?.startsWith('en') ? 'en' : 'zh';

  if (nodes.length === 0) {
    return <p className="text-caption text-ink-400 py-1">{t('timeline.noSteps')}</p>;
  }

  return (
    <div
      className={`flex items-stretch gap-1 overflow-x-auto pb-1 ${compact ? 'pt-0.5' : 'pt-1'}`}
      role="list"
      aria-label={t('audit.timeline')}
    >
      {nodes.map((node, index) => {
        const clickable = Boolean(onNodeClick && (node.docId || node.dept));
        const deptMeta = node.dept ? getDeptMeta(node.dept) : null;
        const deptLabel = deptMeta && lang === 'en' ? deptMeta.shortLabelEn : deptMeta?.shortLabel;

        return (
          <div key={node.id} className="flex items-center shrink-0" role="listitem">
            {index > 0 && <span className="w-3 h-px bg-ink-200 shrink-0 mx-0.5" aria-hidden />}
            <button
              type="button"
              disabled={!clickable}
              onClick={() => clickable && onNodeClick?.(node)}
              className={[
                'flex flex-col items-start text-left rounded-md border px-2 py-1 min-w-[88px] max-w-[140px] transition-colors',
                STATUS_STYLES[node.status],
                clickable ? 'hover:brightness-95 cursor-pointer' : 'cursor-default',
                compact ? 'text-[10px]' : 'text-caption',
              ].join(' ')}
              title={node.sublabel}
            >
              <span className="flex items-center gap-1 w-full min-w-0">
                <span className={`w-1.5 h-1.5 rounded-full shrink-0 ${STATUS_DOT[node.status]}`} />
                <span className="font-mono truncate opacity-70">{node.id}</span>
              </span>
              <span className="truncate w-full font-medium leading-snug">{node.label}</span>
              {deptLabel && <span className="truncate w-full opacity-75">{deptLabel}</span>}
              {node.docId && (
                <span className="truncate w-full font-mono opacity-80">{node.docId}</span>
              )}
              {(node.status === 'failed' || node.status === 'deadlocked') && node.failureReason && (
                <span className="truncate w-full text-[10px] opacity-80 mt-0.5">
                  {node.failureReason}
                </span>
              )}
            </button>
          </div>
        );
      })}
    </div>
  );
}
