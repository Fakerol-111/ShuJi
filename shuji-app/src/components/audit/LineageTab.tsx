import type { LineageTabProps } from './types';
import { LineageTree } from './shared';

export default function LineageTab({
  t,
  lineageDocId,
  onChangeLineageDocId,
  lineage,
  lineageLoading,
  onSearch,
}: LineageTabProps) {
  return (
    <div className="p-3 space-y-2">
      <div className="flex gap-1">
        <input
          type="text"
          placeholder={t('audit.lineagePlaceholder')}
          value={lineageDocId}
          onChange={(e) => onChangeLineageDocId(e.target.value)}
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
      {lineageLoading && <div className="text-caption text-ink-400">{t('common.loading')}</div>}
      {lineage === null && !lineageLoading && (
        <div className="text-caption text-ink-300">{t('audit.lineageHint')}</div>
      )}
      {lineage && <LineageTree node={lineage} depth={0} />}
    </div>
  );
}
