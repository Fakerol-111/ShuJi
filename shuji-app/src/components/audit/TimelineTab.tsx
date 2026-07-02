import { EVENT_COLORS, type TimelineTabProps } from './types';

export default function TimelineTab({
  t,
  data,
  loading,
  error,
  searchText,
  onSearchTextChange,
  onShowDiff,
}: TimelineTabProps) {
  if (loading) {
    return <div className="p-4 text-body text-ink-400 text-center mt-8">{t('common.loading')}</div>;
  }
  if (error) {
    return (
      <div className="p-4">
        <div className="rounded-lg bg-vermillion/10 border border-vermillion/20 px-3 py-2 text-caption text-vermillion">
          {error}
        </div>
      </div>
    );
  }
  if (!data || data.entries.length === 0) {
    return (
      <div className="p-4 text-body text-ink-400 text-center mt-8">{t('audit.noGazette')}</div>
    );
  }

  const filtered = searchText
    ? data.entries.filter((e) =>
        [e.event, e.role, e.doc_id, e.detail].some((v) =>
          v.toLowerCase().includes(searchText.toLowerCase())
        )
      )
    : data.entries;

  return (
    <>
      <div className="px-2 py-1 border-b border-fold/50">
        <input
          type="text"
          placeholder={t('audit.searchPlaceholder')}
          value={searchText}
          onChange={(e) => onSearchTextChange(e.target.value)}
          className="w-full px-2 py-1 text-caption rounded bg-ink-100 border border-fold text-ink-700 placeholder-ink-300 outline-none"
        />
      </div>
      <div className="px-3 py-1.5 border-b border-fold space-y-0.5">
        <div className="text-caption text-ink-500">
          {searchText
            ? `${filtered.length} / ${data.summary.total_events} ${t('audit.entries')}`
            : `${t('audit.totalEntries')} ${data.summary.total_events}`}
        </div>
        <div className="flex flex-wrap gap-1">
          {data.summary.by_event.slice(0, 5).map(([evt, count]) => (
            <span key={evt} className="px-1.5 py-0.5 rounded bg-ink-100 text-caption text-ink-600">
              {t(`audit.${evt}`)} {count}
            </span>
          ))}
        </div>
      </div>
      <div className="flex-1 overflow-y-auto min-h-0">
        {[...filtered].reverse().map((entry, i) => {
          const color = EVENT_COLORS[entry.event] || 'text-ink-500';
          const hasDiff = entry.event === 'modify_document' || entry.event === 'append_document';
          return (
            <div
              key={i}
              className="px-3 py-1.5 border-b border-fold/50 hover:bg-ink-100/30 transition-colors"
            >
              <div className="flex items-center gap-1.5 text-caption">
                <span className={`font-mono ${color}`}>{t(`audit.${entry.event}`)}</span>
                <span className="text-ink-400">{entry.role}</span>
                {entry.doc_id && <span className="text-ink-500 font-mono">{entry.doc_id}</span>}
              </div>
              <div className="text-caption text-ink-400 mt-0.5 truncate">{entry.detail || '—'}</div>
              {hasDiff && entry.doc_id && (
                <div className="mt-0.5">
                  <button
                    onClick={() => onShowDiff?.(entry.doc_id!)}
                    className="text-[10px] text-ink-400 hover:text-ink-600 underline"
                  >
                    {t('audit.viewDiffInCenter')}
                  </button>
                </div>
              )}
              <div className="text-[9px] text-ink-300 font-mono mt-0.5">{entry.ts}</div>
            </div>
          );
        })}
      </div>
    </>
  );
}
