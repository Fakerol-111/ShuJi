import type { DashboardTabProps } from './types';

export default function DashboardTab({
  t,
  data,
  loading,
  verification,
  verifying,
  onVerifyTrail,
}: DashboardTabProps) {
  return (
    <div className="p-3 space-y-3 flex-1 overflow-y-auto min-h-0">
      <div className="rounded border border-fold p-2 space-y-1">
        <div className="text-caption font-semibold text-ink-700">{t('audit.verifyChain')}</div>
        {!verification && !verifying && (
          <button
            onClick={onVerifyTrail}
            className="px-2 py-1 rounded bg-ink-700 text-white text-caption hover:bg-ink-600 text-[10px]"
          >
            {t('audit.verifyChain')}
          </button>
        )}
        {verifying && <div className="text-caption text-ink-400">{t('common.loading')}</div>}
        {verification && (
          <div className="space-y-1">
            <div
              className={`text-caption ${verification.chain_intact ? 'text-jade' : 'text-vermillion'}`}
            >
              {verification.chain_intact ? t('audit.chainIntact') : t('audit.chainTampered')}
            </div>
            <div className="text-[10px] text-ink-500">
              {t('audit.totalEntries')} {verification.total_entries}
              {verification.pre_chain_entries > 0 &&
                ` (${t('audit.preChainEntries', { count: verification.pre_chain_entries })})`}
            </div>
            {verification.broken_links.length > 0 && (
              <div className="text-[10px] text-vermillion">
                {t('audit.brokenLinks')}: {verification.broken_links.map((b) => b.seq).join(', ')}
              </div>
            )}
            <div className="text-[9px] text-ink-300 font-mono break-all">
              {t('audit.lastHash')}: {verification.last_entry_hash.slice(0, 16)}...
            </div>
          </div>
        )}
      </div>
      {loading ? (
        <div className="text-caption text-ink-400">{t('common.loading')}</div>
      ) : !data ? (
        <div className="text-caption text-ink-300">{t('common.noData')}</div>
      ) : (
        <>
          <div className="rounded border border-fold p-2 space-y-1">
            <div className="text-caption font-semibold text-ink-700">{t('audit.eventStats')}</div>
            <div className="text-[10px] text-ink-500">
              {t('audit.totalEntries')} {data.summary.total_events}
            </div>
            <div className="grid grid-cols-2 gap-1 mt-1">
              {data.summary.by_event.slice(0, 6).map(([evt, count]) => (
                <div key={evt} className="flex justify-between text-caption">
                  <span className="text-ink-500">{t(`audit.${evt}`)}</span>
                  <span className="text-ink-700 font-mono">{count}</span>
                </div>
              ))}
            </div>
          </div>
          <div className="rounded border border-fold p-2 space-y-1">
            <div className="text-caption font-semibold text-ink-700">{t('audit.deptActivity')}</div>
            <div className="space-y-1">
              {data.summary.by_role.slice(0, 8).map(([role, count]) => {
                const maxCount = data.summary.by_role[0]?.[1] || 1;
                return (
                  <div key={role} className="flex items-center gap-2">
                    <span className="text-caption text-ink-500 w-16 truncate">{role}</span>
                    <div className="flex-1 h-3 rounded bg-ink-100 overflow-hidden">
                      <div
                        className="h-full rounded bg-ink-400/40"
                        style={{ width: `${(count / maxCount) * 100}%` }}
                      />
                    </div>
                    <span className="text-[10px] text-ink-400 font-mono w-6 text-right">
                      {count}
                    </span>
                  </div>
                );
              })}
            </div>
          </div>
          <div className="rounded border border-fold p-2 space-y-1">
            <div className="text-caption font-semibold text-ink-700">
              {t('audit.eventDistribution')}
            </div>
            <div className="space-y-1">
              {data.summary.by_event.map(([evt, count]) => {
                const maxCount = data.summary.by_event[0]?.[1] || 1;
                const barColor =
                  evt === 'create_document'
                    ? '#3b8b7b'
                    : evt === 'modify_document' || evt === 'append_document'
                      ? '#4a7daa'
                      : evt === 'set_document_status'
                        ? '#b8860b'
                        : evt === 'cancel_agent'
                          ? '#c04040'
                          : evt === 'checkpoint'
                            ? '#5a7a9a'
                            : '#888';
                return (
                  <div key={evt} className="flex items-center gap-2">
                    <span className="text-caption text-ink-500 w-20 truncate">
                      {t(`audit.${evt}`)}
                    </span>
                    <div className="flex-1 h-3 rounded bg-ink-100 overflow-hidden">
                      <div
                        className="h-full rounded"
                        style={{
                          width: `${(count / maxCount) * 100}%`,
                          backgroundColor: barColor,
                          opacity: 0.35,
                        }}
                      />
                    </div>
                    <span className="text-[10px] text-ink-400 font-mono w-6 text-right">
                      {count}
                    </span>
                  </div>
                );
              })}
            </div>
          </div>
        </>
      )}
    </div>
  );
}
