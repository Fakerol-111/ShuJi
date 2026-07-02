import type { TraceTabProps } from './types';
import { DocCard } from './shared';

export default function TraceTab({
  t,
  traceDocId,
  onChangeTraceDocId,
  traceResult,
  traceLoading,
  onTrace,
  onJumpToDocLine,
}: TraceTabProps) {
  return (
    <div className="p-3 space-y-2 flex-1 overflow-y-auto min-h-0">
      <div className="flex gap-1">
        <input
          type="text"
          placeholder={t('audit.tracePlaceholder')}
          value={traceDocId}
          onChange={(e) => onChangeTraceDocId(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === 'Enter' && traceDocId.trim()) onTrace();
          }}
          className="flex-1 px-2 py-1 text-caption rounded bg-ink-100 border border-fold text-ink-700 placeholder-ink-300 outline-none"
        />
        <button
          onClick={onTrace}
          className="px-2 py-1 rounded bg-ink-700 text-white text-caption hover:bg-ink-600"
        >
          {t('audit.trace')}
        </button>
      </div>
      {traceLoading && <div className="text-caption text-ink-400">{t('common.loading')}</div>}
      {traceResult && (
        <div className="space-y-3">
          {traceResult.target && (
            <div>
              <div className="text-caption font-semibold text-ink-700 mb-1">
                {t('audit.currentDoc')}
              </div>
              <DocCard node={traceResult.target} />
            </div>
          )}
          {!traceResult.target && (
            <div className="text-caption text-ink-300">
              {t('audit.docNotFound', { id: traceDocId })}
            </div>
          )}
          {traceResult.target && (
            <button
              onClick={() => onJumpToDocLine(traceDocId)}
              className="text-[10px] text-azure hover:underline"
            >
              在文档线中查看
            </button>
          )}
          {traceResult.upstream.length > 0 && (
            <div>
              <div className="text-caption font-semibold text-ink-700 mb-1">
                {t('audit.referencedBy', { count: traceResult.upstream.length })}
              </div>
              <div className="space-y-1">
                {traceResult.upstream.map((node, i) => (
                  <DocCard key={i} node={node} />
                ))}
              </div>
            </div>
          )}
          {traceResult.downstream.length > 0 && (
            <div>
              <div className="text-caption font-semibold text-ink-700 mb-1">
                {t('audit.references', { count: traceResult.downstream.length })}
              </div>
              <div className="space-y-1">
                {traceResult.downstream.map((node, i) => (
                  <DocCard key={i} node={node} />
                ))}
              </div>
            </div>
          )}
          {traceResult.upstream.length === 0 &&
            traceResult.downstream.length === 0 &&
            traceResult.target && (
              <div className="text-caption text-ink-300">{t('audit.noRelations')}</div>
            )}
        </div>
      )}
    </div>
  );
}
