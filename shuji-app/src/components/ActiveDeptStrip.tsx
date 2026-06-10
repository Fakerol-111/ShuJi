import { getDeptMeta } from '../constants';

interface Props {
  activeDepts: string[];
  planInfo: { batches: { status: string }[] } | null;
}

export default function ActiveDeptStrip({ activeDepts, planInfo }: Props) {
  if (activeDepts.length === 0) return null;
  return (
    <div className="shrink-0 flex items-center gap-1 px-3 py-1 bg-gold/[0.03] border-b border-gold/15 text-caption overflow-x-auto">
      <span className="text-gold/60 font-semibold tracking-wider mr-1 whitespace-nowrap font-serif text-[10px]">
        值事
      </span>
      {activeDepts.map((dept) => {
        const meta = getDeptMeta(dept);
        const color = meta?.color || '#6b7280';
        return (
          <span
            key={dept}
            className="inline-flex items-center gap-1 px-2 py-0.5 rounded text-caption font-serif shadow-sm whitespace-nowrap"
            style={{ backgroundColor: `${color}18`, borderLeft: `2px solid ${color}` }}
          >
            <span
              className="w-1.5 h-1.5 rounded-full animate-pulse shrink-0"
              style={{ backgroundColor: color }}
            />
            <span className="text-ink-700 font-medium">{meta?.shortLabel || dept}</span>
          </span>
        );
      })}
      {planInfo && planInfo.batches.length > 0 && (
        <span className="ml-2 flex items-center gap-1 text-caption text-ink-500">
          <span className="text-ink-400">·</span>
          {planInfo.batches.map((b, i) => (
            <span
              key={i}
              className={`text-[10px] ${b.status === 'done' ? 'text-jade' : b.status === 'current' ? 'text-gold font-medium' : 'text-ink-400'}`}
            >
              {b.status === 'done' ? '✓' : b.status === 'current' ? '◉' : '○'}
            </span>
          ))}
          <span className="text-ink-400 text-[10px]">
            {planInfo.batches.filter((b) => b.status === 'done').length}/{planInfo.batches.length}
          </span>
        </span>
      )}
    </div>
  );
}
