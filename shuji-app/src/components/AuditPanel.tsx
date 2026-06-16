import { useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import {
  getAuditTimeline,
  getDocumentLineage,
  generateDeliveryReport,
  traceDocument,
  verifyAuditTrail,
  type VerificationReport,
} from '../api';
import type { TimelineData, LineageNode, TraceResult } from '../types';
import { formatError } from '../utils/error';
import { DocCard, LineageTree, docIdToPath } from './audit/shared';

const EVENT_COLORS: Record<string, string> = {
  create_document: 'text-jade',
  modify_document: 'text-azure',
  append_document: 'text-azure',
  set_document_status: 'text-gold',
  cancel_agent: 'text-vermillion',
  checkpoint: 'text-info',
  milestone: 'text-ink-500',
};

type SubTab = 'timeline' | 'lineage' | 'trace' | 'report' | 'dashboard';

export default function AuditPanel({
  projectDir,
  onDocSelect,
  onShowDiff,
}: {
  projectDir?: string;
  onDocSelect?: (path: string) => void;
  onShowDiff?: (path: string) => void;
}) {
  const { t } = useTranslation();
  const [tab, setTab] = useState<SubTab>('timeline');
  const [data, setData] = useState<TimelineData | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState('');
  const [searchText, setSearchText] = useState('');
  const [lineageDocId, setLineageDocId] = useState('');
  const [lineage, setLineage] = useState<LineageNode | null>(null);
  const [lineageLoading, setLineageLoading] = useState(false);
  const [report, setReport] = useState<string | null>(null);
  const [reportLoading, setReportLoading] = useState(false);
  const [traceDocId, setTraceDocId] = useState('');
  const [traceResult, setTraceResult] = useState<TraceResult | null>(null);
  const [traceLoading, setTraceLoading] = useState(false);
  const [verification, setVerification] = useState<VerificationReport | null>(null);
  const [verifying, setVerifying] = useState(false);

  const TABS: { key: SubTab; label: string }[] = [
    { key: 'timeline', label: t('audit.timeline') },
    { key: 'lineage', label: t('audit.lineage') },
    { key: 'trace', label: t('audit.trace') },
    { key: 'report', label: t('audit.report') },
    { key: 'dashboard', label: t('audit.dashboard') },
  ];

  // Re-fetch when project changes — projectDir acts as a refresh key
  useEffect(() => {
    setLoading(true);
    setData(null);
    setError('');
    getAuditTimeline()
      .then(setData)
      .catch((e) => setError(formatError(e)))
      .finally(() => setLoading(false));
  }, [projectDir]);

  function handleShowDiff(docId: string) {
    const path = docIdToPath(docId);
    if (onShowDiff) {
      onShowDiff(path);
    } else if (onDocSelect) {
      onDocSelect(path);
    }
  }

  function handleSearchLineage() {
    if (!lineageDocId.trim()) return;
    setLineageLoading(true);
    setLineage(null);
    getDocumentLineage(lineageDocId.trim())
      .then(setLineage)
      .catch((e) => {
        console.error('血缘查询失败', e);
        setLineage(null);
      })
      .finally(() => setLineageLoading(false));
  }

  function handleTrace() {
    if (!traceDocId.trim()) return;
    setTraceLoading(true);
    setTraceResult(null);
    traceDocument(traceDocId.trim())
      .then(setTraceResult)
      .catch((e) => {
        console.error('追溯查询失败', e);
        setTraceResult(null);
      })
      .finally(() => setTraceLoading(false));
  }

  function handleVerifyTrail() {
    setVerifying(true);
    setVerification(null);
    verifyAuditTrail()
      .then(setVerification)
      .catch((e) => {
        console.error('验证审计完整性失败', e);
      })
      .finally(() => setVerifying(false));
  }

  function handleLoadReport() {
    if (report) return;
    setReportLoading(true);
    generateDeliveryReport()
      .then(setReport)
      .catch((e) => {
        console.error('生成交付报告失败', e);
        setReport(t('audit.loadFailed'));
      })
      .finally(() => setReportLoading(false));
  }

  return (
    <div className="h-full flex flex-col">
      <div className="flex border-b border-fold text-caption">
        {TABS.map((t) => (
          <button
            key={t.key}
            onClick={() => setTab(t.key)}
            className={`flex-1 py-1.5 text-center font-medium transition-colors ${tab === t.key ? 'text-ink-700 border-b-2 border-ink-700' : 'text-ink-400 hover:text-ink-600'}`}
          >
            {t.label}
          </button>
        ))}
      </div>

      {/* ── Timeline Tab ── */}
      {tab === 'timeline' && (
        <>
          <div className="px-2 py-1 border-b border-fold/50">
            <input
              type="text"
              placeholder={t('audit.searchPlaceholder')}
              value={searchText}
              onChange={(e) => setSearchText(e.target.value)}
              className="w-full px-2 py-1 text-caption rounded bg-ink-100 border border-fold text-ink-700 placeholder-ink-300 outline-none"
            />
          </div>
          {loading ? (
            <div className="p-4 text-body text-ink-400 text-center mt-8">{t('common.loading')}</div>
          ) : error ? (
            <div className="p-4">
              <div className="rounded-lg bg-vermillion/10 border border-vermillion/20 px-3 py-2 text-caption text-vermillion">
                {error}
              </div>
            </div>
          ) : !data || data.entries.length === 0 ? (
            <div className="p-4 text-body text-ink-400 text-center mt-8">{t('audit.noGazette')}</div>
          ) : (
            (() => {
              const filtered = searchText
                ? data.entries.filter((e) =>
                    [e.event, e.role, e.doc_id, e.detail].some((v) =>
                      v.toLowerCase().includes(searchText.toLowerCase())
                    )
                  )
                : data.entries;
              return (
                <>
                  <div className="px-3 py-1.5 border-b border-fold space-y-0.5">
                    <div className="text-caption text-ink-500">
                      {searchText
                        ? `${filtered.length} / ${data.summary.total_events} ${t('audit.entries')}`
                        : `${t('audit.totalEntries')} ${data.summary.total_events}`}
                    </div>
                    <div className="flex flex-wrap gap-1">
                      {data.summary.by_event.slice(0, 5).map(([evt, count]) => (
                        <span
                          key={evt}
                          className="px-1.5 py-0.5 rounded bg-ink-100 text-caption text-ink-600"
                        >
                          {t(`audit.${evt}`)} {count}
                        </span>
                      ))}
                    </div>
                  </div>
                  <div className="flex-1 overflow-y-auto min-h-0">
                    {[...filtered].reverse().map((entry, i) => {
                      const color = EVENT_COLORS[entry.event] || 'text-ink-500';
                      const hasDiff =
                        entry.event === 'modify_document' || entry.event === 'append_document';
                      return (
                        <div
                          key={i}
                          className="px-3 py-1.5 border-b border-fold/50 hover:bg-ink-100/30 transition-colors"
                        >
                          <div className="flex items-center gap-1.5 text-caption">
                            <span className={`font-mono ${color}`}>
                              {t(`audit.${entry.event}`)}
                            </span>
                            <span className="text-ink-400">{entry.role}</span>
                            {entry.doc_id && (
                              <span className="text-ink-500 font-mono">{entry.doc_id}</span>
                            )}
                          </div>
                          <div className="text-caption text-ink-400 mt-0.5 truncate">
                            {entry.detail || '—'}
                          </div>
                          {hasDiff && entry.doc_id && (
                            <div className="mt-0.5">
                              <button
                                onClick={() => handleShowDiff(entry.doc_id!)}
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
            })()
          )}
        </>
      )}

      {/* ── Lineage Tab ── */}
      {tab === 'lineage' && (
        <div className="p-3 space-y-2">
          <div className="flex gap-1">
            <input
              type="text"
              placeholder={t('audit.lineagePlaceholder')}
              value={lineageDocId}
              onChange={(e) => setLineageDocId(e.target.value)}
              onKeyDown={(e) => e.key === 'Enter' && handleSearchLineage()}
              className="flex-1 px-2 py-1 text-caption rounded bg-ink-100 border border-fold text-ink-700 placeholder-ink-300 outline-none"
            />
            <button
              onClick={handleSearchLineage}
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
      )}

      {/* ── Trace Tab ── */}
      {tab === 'trace' && (
        <div className="p-3 space-y-2 flex-1 overflow-y-auto min-h-0">
          <div className="flex gap-1">
            <input
              type="text"
              placeholder={t('audit.tracePlaceholder')}
              value={traceDocId}
              onChange={(e) => setTraceDocId(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === 'Enter' && traceDocId.trim()) handleTrace();
              }}
              className="flex-1 px-2 py-1 text-caption rounded bg-ink-100 border border-fold text-ink-700 placeholder-ink-300 outline-none"
            />
            <button
              onClick={handleTrace}
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
                  <div className="text-caption font-semibold text-ink-700 mb-1">{t('audit.currentDoc')}</div>
                  <DocCard node={traceResult.target} onDocSelect={onDocSelect} />
                </div>
              )}
              {!traceResult.target && (
                <div className="text-caption text-ink-300">{t('audit.docNotFound', { id: traceDocId })}</div>
              )}
              {traceResult.upstream.length > 0 && (
                <div>
                  <div className="text-caption font-semibold text-ink-700 mb-1">
                    {t('audit.referencedBy', { count: traceResult.upstream.length })}
                  </div>
                  <div className="space-y-1">
                    {traceResult.upstream.map((node, i) => (
                      <DocCard key={i} node={node} onDocSelect={onDocSelect} />
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
                      <DocCard key={i} node={node} onDocSelect={onDocSelect} />
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
      )}

      {/* ── Report Tab ── */}
      {tab === 'report' && (
        <div className="p-3 flex-1 overflow-y-auto min-h-0">
          {!report && !reportLoading && (
            <button
              onClick={handleLoadReport}
              className="px-3 py-1.5 rounded bg-ink-700 text-white text-caption hover:bg-ink-600"
            >
              {t('audit.generateReport')}
            </button>
          )}
          {reportLoading && <div className="text-caption text-ink-400">{t('common.loading')}</div>}
          {report && (
            <div className="text-caption text-ink-700 whitespace-pre-wrap font-mono text-[11px] leading-relaxed">
              {report}
            </div>
          )}
        </div>
      )}

      {/* ── Dashboard Tab ── */}
      {tab === 'dashboard' && (
        <div className="p-3 space-y-3 flex-1 overflow-y-auto min-h-0">
          {/* ── Audit Integrity Check ── */}
          <div className="rounded border border-fold p-2 space-y-1">
            <div className="text-caption font-semibold text-ink-700">{t('audit.verifyChain')}</div>
            {!verification && !verifying && (
              <button
                onClick={handleVerifyTrail}
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
                <div className="text-[10px] text-ink-500">{t('audit.totalEntries')} {data.summary.total_events}</div>
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
                <div className="text-caption font-semibold text-ink-700">{t('audit.eventDistribution')}</div>
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
      )}
    </div>
  );
}
