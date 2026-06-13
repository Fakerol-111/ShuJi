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
          (planInfo.batches.filter((b) => b.status === 'done').length / planInfo.batches.length) *
            100
        )
      : 0;

  return (
    <button
      onClick={onClick}
      aria-selected={isSelected}
      role="tab"
      className={`
        w-full text-left transition-all duration-300
        ${isSelected ? 'bg-ink-100/40 border-l-[3px]' : 'border-l border-transparent'}
        ${!isSelected ? 'hover:bg-ink-100/20' : ''}
      `}
      style={{
        borderLeftColor: isSelected ? meta.color : undefined,
        boxShadow: isActive ? `0 0 12px ${meta.color}22` : undefined,
        backgroundColor: isActive && !isSelected ? `${meta.color}0d` : undefined,
      }}
    >
      <div className="px-3 py-2.5">
        <div className="flex items-center gap-2">
          <span
            className={`w-2 h-2 rounded-full shrink-0 ${isActive ? 'animate-pulse' : ''}`}
            style={{ backgroundColor: meta.color }}
          />
          <span className="text-xs font-semibold text-ink-800 truncate">{meta.shortLabel}</span>
          {hasError && (
            <span className="w-1.5 h-1.5 rounded-full bg-vermillion shrink-0" title="最近出错" />
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
            <span className="text-[9px] text-ink-400 font-mono shrink-0">
              {planInfo!.batches.filter((b) => b.status === 'done').length}/
              {planInfo!.batches.length}
            </span>
          </div>
        )}
        {latestAction && !showPlan && (
          <div className="mt-0.5 text-[10px] text-ink-500 truncate leading-tight">
            {latestAction}
          </div>
        )}
      </div>
    </button>
  );
}
