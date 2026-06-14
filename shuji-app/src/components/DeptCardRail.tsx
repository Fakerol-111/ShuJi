import { DEPT_META_LIST, DEPT_RAIL_GROUPS } from '../constants';
import { isDeptActive } from '../utils/deptLog';
import DeptCard from './DeptCard';
import DeptGlyph from './DeptGlyph';
import type { DeptLogEntry, PlanInfo } from '../types';

const ERROR_PREFIX = '❌';

interface DeptCardRailProps {
  selected: string | null;
  onSelect: (dept: string | null) => void;
  activeDepts: string[];
  latestLogs: Map<string, DeptLogEntry>;
  planInfo?: PlanInfo | null;
  pinDept: boolean;
  onTogglePin: () => void;
}

export default function DeptCardRail({
  selected,
  onSelect,
  activeDepts,
  latestLogs,
  planInfo,
  pinDept,
  onTogglePin,
}: DeptCardRailProps) {
  const metaByLabel = new Map(DEPT_META_LIST.map((d) => [d.label, d]));

  const renderDept = (label: string) => {
    const meta = metaByLabel.get(label);
    if (!meta) return null;
    const active = isDeptActive(label, activeDepts);
    const selectedDept = selected === label;
    const latestEntry = latestLogs.get(label) || latestLogs.get(meta.shortLabel);
    const hasError = latestEntry?.action?.startsWith(ERROR_PREFIX) ?? false;
    const latestAction = latestEntry ? latestEntry.action.replace(/^[❌→]\s*/, '') : '';

    return (
      <DeptCard
        key={label}
        meta={meta}
        isActive={active}
        isSelected={selectedDept}
        hasError={hasError}
        latestAction={latestAction}
        planInfo={planInfo}
        onClick={() => onSelect(selectedDept ? null : label)}
      />
    );
  };

  return (
    <div className="w-[var(--cockpit-rail-width)] shrink-0 overflow-y-auto overflow-x-hidden border-r border-fold bg-surface-parchment/40 flex flex-col">
      <div className="flex-1 min-h-0">
        {DEPT_RAIL_GROUPS.map((group, gi) => (
          <div key={gi}>
            {gi > 0 && <div className="border-b border-fold/50 mx-2" />}
            <div className="px-3 pt-3 pb-1">
              <div className="flex items-baseline gap-2">
                <span className="font-display text-caption font-semibold text-ink-700">{group.title}</span>
                <span className="text-caption text-ink-400">{group.subtitle}</span>
              </div>
              <div className="mt-1 h-px bg-gradient-to-r from-gold/40 to-transparent" />
            </div>
            {group.labels.map(renderDept)}
          </div>
        ))}
      </div>

      <div className="border-t border-fold">
        <button
          onClick={() => onSelect(selected === '__all__' ? null : '__all__')}
          className={`
            w-full text-left px-3 py-2 text-caption font-medium transition-colors
            ${selected === '__all__' ? 'bg-ink-100/40 text-ink-800' : 'text-ink-500 hover:bg-ink-100/20 hover:text-ink-700'}
          `}
        >
          <span className="flex items-center gap-2">
            <DeptGlyph deptKey="" size={16} stroke="#8B7355" />
            诸司动态
          </span>
        </button>
        <button
          onClick={onTogglePin}
          className={`
            w-full text-left px-3 py-1.5 text-caption font-mono transition-colors border-t border-fold
            ${pinDept ? 'text-ink-600 bg-ink-100/20' : 'text-gold hover:bg-ink-100/20'}
          `}
          title={pinDept ? '已固定视角，关闭自动跟随' : '跟随当值部门'}
        >
          <span className="flex items-center gap-2">
            <svg width="16" height="16" viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.5">
              <path d="M10 2L14 6M6 10L2 14M11 3L13 5M9 1L15 7M12 8.5C12 11 9.5 13.5 8 14.5c-1.5-1-4-3.5-4-6C4 5.5 6 3.5 8 3.5s4 2 4 5z" />
              <circle cx="8" cy="8.5" r="1.5" />
            </svg>
            {pinDept ? '已固定视角' : '跟随当值'}
          </span>
        </button>
      </div>
    </div>
  );
}
