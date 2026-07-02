import { docIdToPath } from './shared';
import type { SearchTabProps } from './types';

export default function SearchTab({
  t,
  searchStatus,
  onChangeSearchStatus,
  searchKeyword,
  onChangeSearchKeyword,
  searchResults,
  searchLoading,
  onSearch,
  onQuickFilter,
  onDocSelect,
}: SearchTabProps) {
  return (
    <div className="p-3 space-y-2 flex-1 overflow-y-auto min-h-0">
      <div className="flex flex-wrap gap-1">
        <button
          onClick={() => onQuickFilter('in_review')}
          className="px-2 py-0.5 rounded bg-gold/10 text-gold text-[10px] hover:bg-gold/20"
        >
          全部待批
        </button>
        <button
          onClick={() => onQuickFilter('rejected')}
          className="px-2 py-0.5 rounded bg-vermillion/10 text-vermillion text-[10px] hover:bg-vermillion/20"
        >
          全部已驳回
        </button>
      </div>
      <div className="flex gap-1">
        <select
          value={searchStatus}
          onChange={(e) => onChangeSearchStatus(e.target.value)}
          className="px-2 py-1 text-caption rounded bg-ink-100 border border-fold text-ink-700 outline-none"
        >
          <option value="">全部状态</option>
          <option value="in_review">in_review</option>
          <option value="approved">approved</option>
          <option value="rejected">rejected</option>
        </select>
        <input
          type="text"
          placeholder="关键词"
          value={searchKeyword}
          onChange={(e) => onChangeSearchKeyword(e.target.value)}
          onKeyDown={(e) => e.key === 'Enter' && onSearch()}
          className="flex-1 px-2 py-1 text-caption rounded bg-ink-100 border border-fold text-ink-700 placeholder-ink-300 outline-none"
        />
        <button
          onClick={onSearch}
          className="px-2 py-1 rounded bg-ink-700 text-white text-caption hover:bg-ink-600"
        >
          {t('audit.query')}
        </button>
      </div>
      {searchLoading && <div className="text-caption text-ink-400">{t('common.loading')}</div>}
      {!searchLoading && searchResults.length === 0 && (
        <div className="text-caption text-ink-300">无匹配文档</div>
      )}
      <div className="space-y-1">
        {searchResults.map((doc) => (
          <div
            key={doc.id}
            className="rounded border border-fold p-2 hover:bg-ink-100/30 cursor-pointer transition-colors"
            onClick={() => onDocSelect?.(docIdToPath(doc.id))}
          >
            <div className="flex items-center gap-1.5 text-caption">
              <span className="font-mono text-ink-700">{doc.id}</span>
              <span className="text-ink-400">({doc.doc_type})</span>
              {doc.status && (
                <span
                  className={`px-1 rounded text-[9px] ${doc.status === 'approved' ? 'bg-jade/10 text-jade' : doc.status === 'rejected' ? 'bg-vermillion/10 text-vermillion' : 'bg-gold/10 text-gold'}`}
                >
                  {doc.status}
                </span>
              )}
            </div>
            <div className="text-caption text-ink-400 truncate mt-0.5">{doc.preview || '—'}</div>
            <div className="text-[9px] text-ink-300 font-mono mt-0.5">
              {doc.author} · {doc.timestamp}
            </div>
          </div>
        ))}
      </div>
    </div>
  );
}
