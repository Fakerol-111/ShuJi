import { useEffect, useMemo, useState } from 'react';
import { useTranslation } from 'react-i18next';
import ReactMarkdown from 'react-markdown';
import remarkGfm from 'remark-gfm';
import {
  readShujiDoc,
  setDocumentStatus as apiSetStatus,
  sendMessage,
  getDocumentDiff,
  getDocumentLineage,
} from '../api';
import { formatError } from '../utils/error';
import type { DocumentDiff } from '../api';
import type { LineageNode } from '../types';
import { Card } from './ui/Card';

interface DocPreviewProps {
  projectDir: string;
  docPath: string;
  initialTab?: ViewMode;
  onClose?: () => void;
}

type ViewMode = 'content' | 'diff' | 'lineage';

const REJECTION_REASONS = [
  { labelKey: 'docPreview.reasonNoApi', value: '缺少 API 定义' },
  { labelKey: 'docPreview.reasonNoTest', value: '缺少测试策略' },
  { labelKey: 'docPreview.reasonTooBroad', value: '范围过大需拆分' },
  { labelKey: 'docPreview.reasonCustom', value: '' },
];

export default function DocPreview({ projectDir, docPath, initialTab, onClose }: DocPreviewProps) {
  const { t } = useTranslation();
  const [content, setContent] = useState('');
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState('');
  const [approving, setApproving] = useState(false);
  const [approvalError, setApprovalError] = useState('');
  const [comment, setComment] = useState('');
  const [viewMode, setViewMode] = useState<ViewMode>(initialTab || 'content');
  const [diffData, setDiffData] = useState<DocumentDiff | null>(null);
  const [diffLoading, setDiffLoading] = useState(false);
  const [lineage, setLineage] = useState<LineageNode | null>(null);
  const [lineageLoading, setLineageLoading] = useState(false);

  useEffect(() => {
    setLoading(true);
    setError('');
    setApprovalError('');
    setDiffData(null);
    setLineage(null);
    setViewMode(initialTab || 'content');
    readShujiDoc(projectDir, docPath)
      .then((doc) => setContent(doc.content))
      .catch((e) => setError(formatError(e)))
      .finally(() => setLoading(false));

    // Fetch diff in parallel
    setDiffLoading(true);
    getDocumentDiff(projectDir, docPath)
      .then((d) => setDiffData(d))
      .catch((e) => {
        console.error('获取文档差异失败', e);
      })
      .finally(() => setDiffLoading(false));

    // Fetch lineage for .shuji documents
    if (docPath.startsWith('.shuji/') && docPath.endsWith('.md')) {
      setLineageLoading(true);
      const parsedId = docPath.split('/').pop()?.replace(/\.md$/, '') || '';
      getDocumentLineage(parsedId)
        .then((l) => setLineage(l))
        .catch((e) => {
          console.error('获取文档血缘失败', e);
        })
        .finally(() => setLineageLoading(false));
    }
  }, [projectDir, docPath]);

  const isShujiMarkdown = docPath.startsWith('.shuji/') && docPath.endsWith('.md');
  const isMarkdown = docPath.endsWith('.md');
  const parsed = useMemo(() => parseFrontmatter(content), [content]);
  const parts = docPath.split('/');
  const docId =
    (isShujiMarkdown && parsed.meta?.id) || docPath.split('/').pop()?.replace(/\.md$/, '') || '';
  const docStatus = parsed.meta?.status || '';

  const handleApproval = async (status: 'approved' | 'rejected') => {
    setApproving(true);
    setApprovalError('');
    try {
      const msg =
        status === 'approved'
          ? `朕已御批。${comment ? ' ' + comment : ''}`
          : `驳回。${comment ? ' ' + comment : ''}`;
      // 1. Write judgment to document (must succeed)
      await apiSetStatus(docId, status, comment || undefined);
      // 2. Notify 内阁 (best-effort — judgment is already saved)
      try {
        await sendMessage(msg);
      } catch (e) {
        setApprovalError(t('docPreview.approvalNotifyFailed', { error: formatError(e) }));
        // Still refresh the doc to show updated status
        const doc = await readShujiDoc(projectDir, docPath);
        setContent(doc.content);
        return;
      }
      const doc = await readShujiDoc(projectDir, docPath);
      setContent(doc.content);
    } catch (e) {
      setApprovalError(formatError(e));
    } finally {
      setApproving(false);
    }
  };

  const insertRejectionReason = (reason: string) => {
    if (reason) {
      setComment(reason);
    }
  };

  if (loading) return <div className="p-6 text-body text-ink-400">{t('docPreview.loading')}</div>;
  if (error) return <div className="p-6 text-body text-vermillion">{error}</div>;

  return (
    <div className="h-full overflow-y-auto surface-paper">
      <div className="px-6 py-6 lg:px-8 lg:py-8">
        <div className="flex items-center gap-2 mb-4">
          <div className="text-caption text-ink-400 font-mono flex flex-wrap gap-1 flex-1 min-w-0">
            {parts.map((p, i) => (
              <span key={`${p}-${i}`}>
                {i > 0 && <span className="mx-1 text-ink-300">/</span>}
                {p}
              </span>
            ))}
          </div>
          {onClose && (
            <button
              onClick={onClose}
              className="shrink-0 w-5 h-5 flex items-center justify-center rounded text-caption text-ink-400 hover:text-ink-900 hover:bg-ink-200/60 transition-colors"
              title={t('common.close')}
            >
              ✕
            </button>
          )}
        </div>

        {/* ── View toggle tabs ── */}
        <div className="mb-4 flex gap-1 border-b border-fold">
          <button
            onClick={() => setViewMode('content')}
            className={`px-4 py-2 text-ui font-bold rounded-t-lg transition -mb-px border-b-2 ${
              viewMode === 'content'
                ? 'border-vermillion text-ink-900'
                : 'border-transparent text-ink-400 hover:text-ink-600'
            }`}
          >
            {t('document.fullText')}
          </button>
          {diffData?.has_previous && (
            <button
              onClick={() => setViewMode('diff')}
              className={`px-4 py-2 text-ui font-bold rounded-t-lg transition -mb-px border-b-2 ${
                viewMode === 'diff'
                  ? 'border-vermillion text-ink-900'
                  : 'border-transparent text-ink-400 hover:text-ink-600'
              }`}
            >
              {t('document.diff')}
              <span className="ml-1.5 text-caption text-ink-400">
                {diffData ? `+${diffData.added}/-${diffData.removed}` : ''}
              </span>
            </button>
          )}
          {docPath.startsWith('.shuji/') && docPath.endsWith('.md') && (
            <button
              onClick={() => setViewMode('lineage')}
              className={`px-4 py-2 text-ui font-bold rounded-t-lg transition -mb-px border-b-2 ${
                viewMode === 'lineage'
                  ? 'border-vermillion text-ink-900'
                  : 'border-transparent text-ink-400 hover:text-ink-600'
              }`}
            >
              {t('document.lineage')}
            </button>
          )}
        </div>

        {/* ── "待陛下朱批" banner ── */}
        {docStatus === 'in_review' && (
          <div className="mb-4 rounded-xl border border-vermillion/30 bg-surface-elevated p-4 shadow-sm">
            <div className="flex items-center justify-between">
              <div>
                <h3 className="font-display text-sm font-bold text-ink-900">
                  {t('document.pendingApproval')}
                </h3>
                <p className="text-caption text-ink-600 mt-0.5">{t('document.approvalRequired')}</p>
              </div>
              <div className="flex gap-2">
                <button
                  onClick={() => handleApproval('approved')}
                  disabled={approving}
                  className="bg-jade hover:bg-jade/80 text-white text-ui font-bold px-4 py-2 rounded-lg transition disabled:opacity-50"
                >
                  {approving ? t('common.loading') : t('document.approve')}
                </button>
                <button
                  onClick={() => handleApproval('rejected')}
                  disabled={approving}
                  className="bg-vermillion hover:bg-vermillion-dark text-white text-ui font-bold px-4 py-2 rounded-lg transition disabled:opacity-50"
                >
                  {t('document.reject')}
                </button>
              </div>
            </div>
            <div className="mt-2 flex gap-2">
              <input
                type="text"
                placeholder={t('document.imperialNote')}
                value={comment}
                onChange={(e) => setComment(e.target.value)}
                className="flex-1 px-3 py-1.5 border border-fold rounded-lg text-body bg-surface-parchment"
              />
              <select
                onChange={(e) => insertRejectionReason(e.target.value)}
                value=""
                className="px-2 py-1.5 border border-fold rounded-lg text-caption bg-surface-parchment text-ink-600"
              >
                <option value="" disabled>
                  {t('document.rejectionTemplate')}
                </option>
                {REJECTION_REASONS.map((r) => (
                  <option key={r.value || '__custom'} value={r.value}>
                    {t(r.labelKey)}
                  </option>
                ))}
              </select>
            </div>
            {approvalError && <p className="text-caption text-vermillion mt-1">{approvalError}</p>}
          </div>
        )}

        {viewMode === 'lineage' ? (
          lineageLoading ? (
            <div className="p-6 text-body text-ink-400">{t('docPreview.loadingLineage')}</div>
          ) : lineage ? (
            <LineageTree node={lineage} depth={0} />
          ) : (
            <div className="p-6 text-body text-ink-400 text-center">
              {t('docPreview.noLineage')}
            </div>
          )
        ) : viewMode === 'diff' && diffData ? (
          <DiffView diff={diffData.diff} />
        ) : diffLoading ? (
          <div className="p-6 text-body text-ink-400">{t('docPreview.loadingDiff')}</div>
        ) : (
          <>
            {isShujiMarkdown && parsed.meta && <FrontmatterCard meta={parsed.meta} />}
            {isMarkdown ? (
              <article className="prose prose-shuji max-w-none">
                <ReactMarkdown remarkPlugins={[remarkGfm]}>
                  {(isShujiMarkdown ? parsed.body : content) || t('docPreview.fileEmpty')}
                </ReactMarkdown>
              </article>
            ) : (
              <CodePreview content={content} path={docPath} />
            )}
          </>
        )}
      </div>
    </div>
  );
}

function DiffView({ diff }: { diff: string }) {
  const { t } = useTranslation();
  if (!diff) {
    return <div className="p-6 text-body text-ink-400 text-center">{t('docPreview.noDiff')}</div>;
  }

  const lines = diff.split('\n');

  return (
    <div
      className="rounded-xl border overflow-hidden shadow-sm"
      style={{
        borderColor: 'var(--code-border)',
        backgroundColor: 'var(--code-bg)',
      }}
    >
      <div
        className="h-9 flex items-center px-3 text-[11px] font-mono"
        style={{
          backgroundColor: 'var(--code-tab-bg)',
          borderBottom: '1px solid var(--code-border)',
          color: 'var(--code-muted)',
        }}
      >
        <span>Unified Diff</span>
      </div>
      <div className="overflow-auto max-h-[calc(100vh-190px)] text-[13px] leading-[22px] font-[Cascadia_Code,JetBrains_Mono,Consolas,Menlo,Monaco,monospace]">
        <table className="w-full border-separate border-spacing-0">
          <tbody>
            {lines.map((line, i) => {
              let bgColor = 'transparent';
              let textColor = 'var(--code-text)';
              if (line.startsWith('+') && !line.startsWith('+++')) {
                bgColor = 'rgba(34,197,94,0.10)';
                textColor = '#16a34a';
              } else if (line.startsWith('-') && !line.startsWith('---')) {
                bgColor = 'rgba(239,68,68,0.10)';
                textColor = '#dc2626';
              } else if (line.startsWith('@@')) {
                textColor = 'var(--code-line-num)';
              } else if (line.startsWith('---') || line.startsWith('+++')) {
                textColor = 'var(--code-line-num)';
              }
              return (
                <tr key={i} style={{ backgroundColor: bgColor }}>
                  <td className="pl-4 pr-6 whitespace-pre align-top" style={{ color: textColor }}>
                    {line || ' '}
                  </td>
                </tr>
              );
            })}
          </tbody>
        </table>
      </div>
    </div>
  );
}

function CodePreview({ content, path }: { content: string; path: string }) {
  const { t } = useTranslation();
  const lines = (content || t('docPreview.fileEmpty')).split(/\r?\n/);
  const language = languageName(path);

  return (
    <div
      className="rounded-xl border shadow-sm overflow-hidden"
      style={{
        borderColor: 'var(--code-border)',
        backgroundColor: 'var(--code-bg)',
      }}
    >
      <div
        className="h-9 flex items-center justify-between text-[11px]"
        style={{
          backgroundColor: 'var(--code-tab-bg)',
          borderBottom: '1px solid var(--code-border)',
        }}
      >
        <div
          className="h-full px-3 flex items-center gap-2 font-mono"
          style={{
            backgroundColor: 'var(--code-bg)',
            borderRight: '1px solid var(--code-border)',
            color: 'var(--code-text)',
          }}
        >
          <span style={{ color: 'var(--code-muted)' }}>{fileGlyph(path)}</span>
          <span className="truncate max-w-[520px]">{path.split('/').pop()}</span>
        </div>
        <div
          className="px-3 font-mono flex items-center gap-3"
          style={{ color: 'var(--code-muted)' }}
        >
          <span>{language}</span>
          <span>{lines.length.toLocaleString()} lines</span>
          <span>{content.length.toLocaleString()} chars</span>
        </div>
      </div>
      <div className="overflow-auto max-h-[calc(100vh-190px)] text-[13px] leading-[22px] font-[Cascadia_Code,JetBrains_Mono,Consolas,Menlo,Monaco,monospace]">
        <table className="w-full border-separate border-spacing-0">
          <tbody>
            {lines.map((line, index) => (
              <tr key={index} className="code-preview-row">
                <td
                  className="select-none sticky left-0 w-14 min-w-14 pr-3 text-right align-top"
                  style={{
                    backgroundColor: 'var(--code-bg)',
                    color: 'var(--code-line-num)',
                    borderRight: '1px solid var(--code-border)',
                  }}
                >
                  {index + 1}
                </td>
                <td
                  className="pl-4 pr-6 whitespace-pre align-top"
                  style={{ color: 'var(--code-text)' }}
                >
                  {line || ' '}
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </div>
  );
}

function languageName(path: string) {
  const ext = path.split('.').pop()?.toLowerCase();
  const map: Record<string, string> = {
    rs: 'Rust',
    ts: 'TypeScript',
    tsx: 'TSX',
    js: 'JavaScript',
    jsx: 'JSX',
    json: 'JSON',
    jsonl: 'JSONL',
    toml: 'TOML',
    yaml: 'YAML',
    yml: 'YAML',
    css: 'CSS',
    html: 'HTML',
    py: 'Python',
    sh: 'Shell',
    ps1: 'PowerShell',
    svg: 'SVG',
    txt: 'Text',
    env: 'Env',
  };
  return ext ? map[ext] || ext.toUpperCase() : 'Text';
}

function fileGlyph(path: string) {
  const ext = path.split('.').pop()?.toLowerCase();
  if (['ts', 'tsx', 'js', 'jsx'].includes(ext || '')) return 'TS';
  if (ext === 'rs') return 'RS';
  if (['json', 'jsonl'].includes(ext || '')) return '{}';
  if (['toml', 'yaml', 'yml', 'env'].includes(ext || '')) return '⚙';
  if (ext === 'py') return 'PY';
  return 'TXT';
}

function FrontmatterCard({ meta }: { meta: Record<string, string> }) {
  const { t } = useTranslation();
  const labels: Record<string, string> = {
    id: 'ID',
    type: t('document.type'),
    author: t('document.author'),
    timestamp: t('document.time'),
    refs: t('document.refs'),
    status: t('document.status'),
  };
  return (
    <Card variant="parchment" className="mb-5 border-l-vermillion border-l-[3px] p-4">
      <div className="font-display text-ui text-ink-600 font-semibold mb-2">
        {t('document.ticket')}
      </div>
      <div className="grid grid-cols-1 sm:grid-cols-2 gap-2">
        {Object.entries(meta).map(([key, value]) => {
          const statusColor =
            key === 'status' && value === 'in_review'
              ? 'text-vermillion font-bold'
              : key === 'status' && value === 'approved'
                ? 'text-jade font-bold'
                : key === 'status' && value === 'rejected'
                  ? 'text-vermillion/60 font-bold'
                  : 'text-ink-700';
          if (key === 'notes' && !value) return null;
          if (key === 'status' && !value) return null;
          return (
            <div key={key} className="flex text-ui font-mono">
              <span className="w-20 shrink-0 text-ink-400">{labels[key] || key}</span>
              <span className={`break-all ${statusColor}`}>{value}</span>
            </div>
          );
        })}
      </div>
    </Card>
  );
}

function parseFrontmatter(raw: string): {
  meta: Record<string, string> | null;
  body: string;
} {
  const match = raw.match(/^---\r?\n([\s\S]*?)\r?\n---\r?\n?/);
  if (!match) return { meta: null, body: raw };
  const header = match[1];
  const body = raw.slice(match[0].length).trimStart();
  const meta: Record<string, string> = {};
  for (const line of header.split(/\r?\n/)) {
    const idx = line.indexOf(':');
    if (idx > 0) meta[line.slice(0, idx).trim()] = line.slice(idx + 1).trim();
  }
  return { meta, body };
}

function LineageTree({ node, depth }: { node: LineageNode; depth: number }) {
  const statusColor =
    node.status === 'in_review'
      ? 'text-vermillion'
      : node.status === 'approved'
        ? 'text-jade'
        : node.status === 'rejected'
          ? 'text-vermillion/60'
          : 'text-ink-500';

  return (
    <div className="font-mono text-caption">
      <div className="flex items-center gap-2 py-1" style={{ paddingLeft: `${depth * 20}px` }}>
        {depth > 0 && <span className="text-ink-300 shrink-0">└─</span>}
        <span className="font-bold text-ink-800">{node.id}</span>
        <span className="text-ink-400">({node.doc_type})</span>
        <span className="text-ink-400">— {node.author}</span>
        {node.status && <span className={statusColor}>{node.status}</span>}
      </div>
      <div className="text-[9px] text-ink-400" style={{ paddingLeft: `${depth * 20 + 16}px` }}>
        {node.timestamp}
        {node.refs.length > 0 && ` · 引用: [${node.refs.join(', ')}]`}
      </div>
      {node.children.map((child) => (
        <LineageTree key={child.id} node={child} depth={depth + 1} />
      ))}
    </div>
  );
}
