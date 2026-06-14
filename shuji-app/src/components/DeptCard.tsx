import DeptGlyph from './DeptGlyph';
import type { DeptMeta } from '../constants';
import type { PlanInfo } from '../types';

interface DeptCardProps {
  meta: DeptMeta;
  isActive: boolean;
  isSelected: boolean;
  hasError: boolean;
  latestAction: string;
  planInfo?: PlanInfo | null;
  onClick: () => void;
}

export default function DeptCard({
  meta,
  isActive,
  isSelected,
  hasError,
  latestAction,
  planInfo,
  onClick,
}: DeptCardProps) {
  const showPlan = meta.key === 'gongbushangshu' && planInfo && planInfo.batches.length > 0;
  const progress =
    showPlan && planInfo
      ? Math.round(
          (planInfo.batches.filter((b) => b.status === 'done').length / planInfo.batches.length) * 100
        )
      : 0;

  const bgClass = isSelected
    ? `${meta.tintClass} ring-1 ring-offset-0`
    : isActive
      ? meta.tintActiveClass
      : 'bg-surface-elevated/60';

  return (
    <button
      onClick={onClick}
      aria-selected={isSelected}
      role="tab"
      className={`w-[calc(100%-1rem)] mx-2 mb-1.5 rounded-xl border border-fold/80 transition-all duration-200 overflow-hidden ${bgClass}`}
      style={{ borderLeftWidth: 3, borderLeftColor: meta.color }}
    >
      <div className="px-3 py-2.5">
        <div className="flex items-center gap-2">
          <DeptGlyph deptKey={meta.key} size={16} stroke={meta.color} />
          <span className="text-ui font-display font-semibold text-ink-800 truncate">{meta.shortLabel}</span>
          {isActive && (
            <span className="w-1.5 h-1.5 rounded-full bg-gold animate-pulse shrink-0 ml-auto" />
          )}
          {hasError && (
            <span className="w-1.5 h-1.5 rounded-full bg-vermillion shrink-0 ml-auto" title="最近出错" />
          )}
        </div>
        {showPlan && (
          <div className="mt-1.5 flex items-center gap-1.5">
            <div className="flex-1 h-1 bg-ink-200 rounded-full overflow-hidden">
              <div
                className="h-full rounded-full transition-all duration-500"
                style={{ width: `${progress}%`, backgroundColor: meta.color }}
              />
            </div>
            <span className="text-caption text-ink-400 font-mono shrink-0">
              {planInfo!.batches.filter((b) => b.status === 'done').length}/
              {planInfo!.batches.length}
            </span>
          </div>
        )}
        {latestAction && !showPlan && (
          <div className="mt-0.5 text-caption text-ink-600 truncate leading-tight">
            {latestAction}
          </div>
        )}
      </div>
    </button>
  );
}
