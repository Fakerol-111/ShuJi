import { getDeptMeta } from '../constants';
import { extractDocPath, stripActionPrefix, classifyDeptAction } from '../utils/deptLog';
import type { DeptLogEntry } from '../types';

const ACTION_CLASS_COLORS: Record<string, string> = {
  output: '#2F7A4F',
  error: '#B83A3A',
  route: '#A16207',
};

interface DeptActivityCardProps {
  entry: DeptLogEntry;
  onDocClick?: (docPath: string) => void;
}

export default function DeptActivityCard({ entry, onDocClick }: DeptActivityCardProps) {
  const meta = getDeptMeta(entry.dept);
  const actionClass = classifyDeptAction(entry);
  const borderColor = ACTION_CLASS_COLORS[actionClass] || meta?.color || '#8B7355';
  const label = meta?.shortLabel || entry.dept;
  const docPath = extractDocPath(entry);

  return (
    <div className="flex items-start gap-2 py-1.5 px-2 rounded-lg hover:bg-ink-100/30 transition-colors group">
        <div
          className="w-[3px] h-full min-h-[20px] rounded-full shrink-0 mt-1"
          style={{ backgroundColor: borderColor }}
        />
      <div className="min-w-0 flex-1">
        <div className="flex items-center gap-1.5 text-caption">
          <span className="font-semibold text-ink-700 shrink-0" style={{ color: borderColor }}>
            {label}
          </span>
          <span className="text-ink-400">·</span>
          <span className="text-ink-400 text-caption shrink-0">{entry.ts}</span>
        </div>
        <div className="text-caption text-ink-600 truncate mt-0.5">
          {stripActionPrefix(entry.action)}
        </div>
        {docPath && onDocClick && (
          <button
            onClick={() => onDocClick(docPath)}
            className="text-caption text-gold hover:text-gold-dark mt-0.5 transition-opacity font-mono"
          >
            查看 → {docPath.split('/').pop()}
          </button>
        )}
      </div>
    </div>
  );
}
