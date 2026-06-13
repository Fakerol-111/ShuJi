import { DEPT_META_LIST, DEPT_ORDER } from '../constants';
import { isDeptActive } from '../utils/deptLog';
import DeptCard from './DeptCard';
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
  return (
    <div className="w-72 shrink-0 overflow-y-auto border-r border-fold bg-surface-paper/50 flex flex-col">
      <div className="flex-1">
        {DEPT_ORDER.map((label) => {
          const meta = DEPT_META_LIST.find((d) => d.label === label);
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
        })}
      </div>

      <div className="border-t border-fold">
        <button
          onClick={() => onSelect(selected === '__all__' ? null : '__all__')}
          className={`
            w-full text-left px-3 py-2 text-xs font-medium transition-colors
            ${selected === '__all__' ? 'bg-ink-100/40 text-ink-800' : 'text-ink-500 hover:bg-ink-100/20 hover:text-ink-700'}
          `}
        >
          <span className="flex items-center gap-2">
            <span className="w-2 h-2 rounded-full bg-ink-400" />
            全部动态
          </span>
        </button>
        <button
          onClick={onTogglePin}
          className={`
            w-full text-left px-3 py-1.5 text-[10px] font-mono transition-colors border-t border-fold
            ${pinDept ? 'text-ink-400 bg-ink-100/20' : 'text-gold hover:bg-ink-100/20'}
          `}
          title={pinDept ? '已固定部门，关闭自动跟随' : '点击固定当前部门，关闭自动跟随'}
        >
          {pinDept ? '📌 已固定' : '跟随活跃'}
        </button>
      </div>
    </div>
  );
}
