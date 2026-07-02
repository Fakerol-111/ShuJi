import { useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import {
  getAuditTimeline,
  getDocumentLineage,
  generateDeliveryReport,
  traceDocument,
  verifyAuditTrail,
  queryDocuments,
  getDocumentLineRun,
  getDocumentLineForDoc,
  listDocumentLineRuns,
} from '../api';
import type {
  TimelineData,
  LineageNode,
  TraceResult,
  DocSummary,
  DocQuery,
  DocumentLineRun,
  LineNode,
} from '../types';
import type { VerificationReport } from '../api';
import { formatError } from '../utils/error';
import { docIdToPath } from './audit/shared';
import type { SubTab } from './audit/types';
import TimelineTab from './audit/TimelineTab';
import LineageTab from './audit/LineageTab';
import TraceTab from './audit/TraceTab';
import DocumentLineTab from './audit/DocumentLineTab';
import SearchTab from './audit/SearchTab';
import ReportTab from './audit/ReportTab';
import DashboardTab from './audit/DashboardTab';

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
  const [searchStatus, setSearchStatus] = useState('');
  const [searchKeyword, setSearchKeyword] = useState('');
  const [searchResults, setSearchResults] = useState<DocSummary[]>([]);
  const [searchLoading, setSearchLoading] = useState(false);
  const [docLineRuns, setDocLineRuns] = useState<string[]>([]);
  const [docLineRunId, setDocLineRunId] = useState('');
  const [docLineDocId, setDocLineDocId] = useState('');
  const [docLine, setDocLine] = useState<DocumentLineRun | null>(null);
  const [docLineLoading, setDocLineLoading] = useState(false);

  const TABS: { key: SubTab; label: string }[] = [
    { key: 'timeline', label: t('audit.timeline') },
    { key: 'lineage', label: t('audit.lineage') },
    { key: 'trace', label: t('audit.trace') },
    { key: 'docline', label: '文档线' },
    { key: 'search', label: '检索' },
    { key: 'report', label: t('audit.report') },
    { key: 'dashboard', label: t('audit.dashboard') },
  ];

  useEffect(() => {
    setLoading(true);
    setData(null);
    setError('');
    getAuditTimeline()
      .then(setData)
      .catch((e) => setError(formatError(e)))
      .finally(() => setLoading(false));
    listDocumentLineRuns()
      .then((runs) => {
        setDocLineRuns(runs);
        if (runs.length > 0 && !docLineRunId) setDocLineRunId(runs[runs.length - 1]);
      })
      .catch(() => setDocLineRuns([]));
  }, [projectDir]);

  function handleLoadDocLine(runId?: string, focusDocId?: string) {
    setDocLineLoading(true);
    setDocLine(null);
    const load = focusDocId?.trim()
      ? getDocumentLineForDoc(focusDocId.trim())
      : getDocumentLineRun(runId?.trim() || undefined);
    load
      .then(setDocLine)
      .catch(() => setDocLine(null))
      .finally(() => setDocLineLoading(false));
  }

  function handleDocLineNodeClick(node: LineNode) {
    if (node.kind === 'document') {
      onDocSelect?.(docIdToPath(node.label));
    } else if (node.kind === 'diff') {
      const diffRef = node.evidence.find((e) => e.source === 'diff_filename');
      if (diffRef) handleShowDiff(node.label.split(' ')[0]);
    }
  }

  function handleShowDiff(docId: string) {
    const path = docIdToPath(docId);
    if (onShowDiff) onShowDiff(path);
    else if (onDocSelect) onDocSelect(path);
  }

  function handleSearchLineage() {
    if (!lineageDocId.trim()) return;
    setLineageLoading(true);
    setLineage(null);
    getDocumentLineage(lineageDocId.trim())
      .then(setLineage)
      .catch(() => setLineage(null))
      .finally(() => setLineageLoading(false));
  }

  function handleTrace() {
    if (!traceDocId.trim()) return;
    setTraceLoading(true);
    setTraceResult(null);
    traceDocument(traceDocId.trim())
      .then(setTraceResult)
      .catch(() => setTraceResult(null))
      .finally(() => setTraceLoading(false));
  }

  function handleVerifyTrail() {
    setVerifying(true);
    setVerification(null);
    verifyAuditTrail()
      .then(setVerification)
      .catch(() => {})
      .finally(() => setVerifying(false));
  }

  function handleLoadReport() {
    if (report) return;
    setReportLoading(true);
    generateDeliveryReport()
      .then(setReport)
      .catch(() => setReport(t('audit.loadFailed')))
      .finally(() => setReportLoading(false));
  }

  function runDocSearch(filter: DocQuery) {
    setSearchLoading(true);
    queryDocuments(filter)
      .then(setSearchResults)
      .catch(() => setSearchResults([]))
      .finally(() => setSearchLoading(false));
  }

  function handleDocSearch() {
    const filter: DocQuery = { limit: 50 };
    if (searchStatus) filter.status = [searchStatus];
    if (searchKeyword.trim()) filter.keyword = searchKeyword.trim();
    runDocSearch(filter);
  }

  function jumpToDocLine(docId: string) {
    setTab('docline');
    setDocLineDocId(docId);
    handleLoadDocLine(undefined, docId);
  }

  const shared = { t, onDocSelect, onShowDiff: handleShowDiff };

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

      {tab === 'timeline' && (
        <TimelineTab
          {...shared}
          data={data}
          loading={loading}
          error={error}
          searchText={searchText}
          onSearchTextChange={setSearchText}
        />
      )}
      {tab === 'lineage' && (
        <LineageTab
          {...shared}
          lineageDocId={lineageDocId}
          onChangeLineageDocId={setLineageDocId}
          lineage={lineage}
          lineageLoading={lineageLoading}
          onSearch={handleSearchLineage}
        />
      )}
      {tab === 'trace' && (
        <TraceTab
          {...shared}
          traceDocId={traceDocId}
          onChangeTraceDocId={setTraceDocId}
          traceResult={traceResult}
          traceLoading={traceLoading}
          onTrace={handleTrace}
          onJumpToDocLine={jumpToDocLine}
        />
      )}
      {tab === 'docline' && (
        <DocumentLineTab
          {...shared}
          docLineRuns={docLineRuns}
          docLineRunId={docLineRunId}
          onChangeDocLineRunId={setDocLineRunId}
          docLineDocId={docLineDocId}
          onChangeDocLineDocId={setDocLineDocId}
          docLine={docLine}
          docLineLoading={docLineLoading}
          onLoadDocLine={handleLoadDocLine}
          onDocLineNodeClick={handleDocLineNodeClick}
        />
      )}
      {tab === 'search' && (
        <SearchTab
          {...shared}
          searchStatus={searchStatus}
          onChangeSearchStatus={setSearchStatus}
          searchKeyword={searchKeyword}
          onChangeSearchKeyword={setSearchKeyword}
          searchResults={searchResults}
          searchLoading={searchLoading}
          onSearch={handleDocSearch}
          onQuickFilter={(status) => {
            setSearchStatus(status);
            runDocSearch({ status: [status], limit: 50 });
          }}
        />
      )}
      {tab === 'report' && (
        <ReportTab
          {...shared}
          report={report}
          reportLoading={reportLoading}
          onLoadReport={handleLoadReport}
        />
      )}
      {tab === 'dashboard' && (
        <DashboardTab
          {...shared}
          data={data}
          loading={loading}
          verification={verification}
          verifying={verifying}
          onVerifyTrail={handleVerifyTrail}
        />
      )}
    </div>
  );
}
