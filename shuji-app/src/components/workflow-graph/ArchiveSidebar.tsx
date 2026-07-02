import type { ArchiveEntry } from './types';

export default function ArchiveSidebar({
  archives,
  activeArchive,
  currentSession,
  switchToLive,
  loadArchivedGraph,
  t,
}: {
  archives: ArchiveEntry[];
  activeArchive: string | null;
  currentSession: string | null;
  switchToLive: () => void;
  loadArchivedGraph: (filename: string) => void;
  t: (key: string) => string;
}) {
  return (
    <div className="w-48 shrink-0 border-r border-fold overflow-y-auto bg-surface-parchment/50">
      <div className="px-3 py-2 border-b border-fold">
        <button
          onClick={switchToLive}
          className={`w-full text-left px-2 py-1.5 rounded text-ui transition-colors ${
            !activeArchive
              ? 'bg-gold-light text-ink-800 font-medium border-l-2 border-gold'
              : 'text-ink-600 hover:bg-ink-100'
          }`}
        >
          <div className="text-xs font-semibold">{t('workflowGraph.current')}</div>
          <div className="text-caption text-ink-500 truncate">
            {currentSession || t('workflowGraph.live')}
          </div>
          {!activeArchive && (
            <span className="inline-block w-1.5 h-1.5 rounded-full bg-jade animate-pulse ml-1" />
          )}
        </button>
      </div>
      <div className="px-3 py-2">
        <div className="text-caption font-semibold text-ink-500 mb-1">
          {t('workflowGraph.history')}
        </div>
        {archives.length === 0 && (
          <p className="text-caption text-ink-400 italic">{t('workflowGraph.noArchives')}</p>
        )}
        {archives.map((a) => (
          <button
            key={a.filename}
            onClick={() => loadArchivedGraph(a.filename)}
            className={`w-full text-left px-2 py-1.5 rounded text-ui transition-colors mb-0.5 ${
              activeArchive === a.filename
                ? 'bg-ink-100 text-ink-800 font-medium border-l-2 border-vermillion'
                : 'text-ink-600 hover:bg-ink-100'
            }`}
          >
            <div className="text-caption truncate">{a.label || t('workflowGraph.unnamed')}</div>
            <div className="text-caption text-ink-400 font-mono text-[10px]">{a.ts}</div>
          </button>
        ))}
      </div>
    </div>
  );
}
