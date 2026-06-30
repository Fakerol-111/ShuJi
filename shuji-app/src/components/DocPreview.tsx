import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import ReactMarkdown from 'react-markdown';
import remarkGfm from 'remark-gfm';
import rehypeHighlight from 'rehype-highlight';
import { listen } from '@tauri-apps/api/event';
import {
  readShujiDoc,
  setDocumentStatus as apiSetStatus,
  sendMessage,
  getDocumentDiff,
  getDocumentDiffs,
  readDocumentDiff,
  getDocumentLineage,
  openInExternalEditor,
} from '../api';
import { formatError } from '../utils/error';
import { useEditorConfig } from '../hooks/useEditorConfig';
import { openInEditorLabel, openLineInEditorLabel } from '../utils/editorLabel';
import { basenameFromPath, splitPathParts } from '../utils/pathBasename';
import type { DocumentDiff } from '../api';
import type { LineageNode } from '../types';

interface DocPreviewProps {
  projectDir: string;
  docPath: string;
  initialTab?: ViewMode;
}

type ViewMode = 'content' | 'diff' | 'lineage';

function countPatchStats(patch: string): { added: number; removed: number } {
  let added = 0;
  let removed = 0;
  for (const line of patch.split('\n')) {
    if (line.startsWith('+') && !line.startsWith('+++')) added++;
    else if (line.startsWith('-') && !line.startsWith('---')) removed++;
  }
  return { added, removed };
}

async function loadAuditDiff(docId: string): Promise<DocumentDiff | null> {
  const diffs = await getDocumentDiffs(docId);
  if (diffs.length === 0) return null;
  const latest = diffs.reduce((a, b) => (a.ts >= b.ts ? a : b));
  const patch = await readDocumentDiff(latest.filename);
  if (!patch.trim()) return null;
  const { added, removed } = countPatchStats(patch);
  return { diff: patch, has_previous: true, added, removed };
}

function docIdFromPath(docPath: string, metaId?: string): string {
  if (metaId) return metaId;
  return basenameFromPath(docPath).replace(/\.md$/, '') || '';
}

export default function DocPreview({ projectDir, docPath, initialTab }: DocPreviewProps) {
  const { t } = useTranslation();
  const editorConfig = useEditorConfig();
  const openInEditorText = useMemo(() => openInEditorLabel(editorConfig, t), [editorConfig, t]);
  const openLineInEditorText = useMemo(
    () => openLineInEditorLabel(editorConfig, t),
    [editorConfig, t]
  );
  const [content, setContent] = useState('');
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState('');
  const [approving, setApproving] = useState(false);
  const [approvalError, setApprovalError] = useState('');
  const [comment, setComment] = useState('');
  const [viewMode, setViewMode] = useState<ViewMode>(initialTab || 'content');
  const [diffData, setDiffData] = useState<DocumentDiff | null>(null);
  const [diffLoading, setDiffLoading] = useState(false);
  const [diffSource, setDiffSource] = useState<'audit' | 'git' | null>(null);
  const [lineage, setLineage] = useState<LineageNode | null>(null);
  const [lineageLoading, setLineageLoading] = useState(false);
  const [editorError, setEditorError] = useState('');
  const [openingEditor, setOpeningEditor] = useState(false);
  const contentRef = useRef('');

  const isShujiMarkdown = docPath.startsWith('.shuji/') && docPath.endsWith('.md');
  const isMarkdown = docPath.endsWith('.md');
  const parsed = useMemo(() => parseFrontmatter(content), [content]);
  const parts = splitPathParts(docPath);
  const docId = isShujiMarkdown ? docIdFromPath(docPath, parsed.meta?.id) : '';
  const docStatus = parsed.meta?.status || '';

  const loadDoc = useCallback(
    async (silent = false) => {
      if (!silent) {
        setLoading(true);
        setError('');
      }
      try {
        const doc = await readShujiDoc(projectDir, docPath);
        contentRef.current = doc.content;
        setContent(doc.content);
      } catch (e) {
        if (!silent) setError(formatError(e));
      } finally {
        if (!silent) setLoading(false);
      }
    },
    [projectDir, docPath]
  );

  const loadDiff = useCallback(
    async (silent = false, metaId?: string) => {
      if (!silent) setDiffLoading(true);
      try {
        if (isShujiMarkdown) {
          const id = docIdFromPath(docPath, metaId);
          const auditDiff = await loadAuditDiff(id);
          setDiffData(auditDiff);
          setDiffSource(auditDiff ? 'audit' : null);
        } else {
          const gitDiff = await getDocumentDiff(projectDir, docPath);
          setDiffData(gitDiff.has_previous ? gitDiff : null);
          setDiffSource(gitDiff.has_previous ? 'git' : null);
        }
      } catch (e) {
        console.error('获取文档差异失败', e);
        if (!silent) {
          setDiffData(null);
          setDiffSource(null);
        }
      } finally {
        if (!silent) setDiffLoading(false);
      }
    },
    [projectDir, docPath, isShujiMarkdown]
  );

  const loadLineage = useCallback(
    async (silent = false, metaId?: string) => {
      if (!isShujiMarkdown) return;
      if (!silent) setLineageLoading(true);
      try {
        const id = docIdFromPath(docPath, metaId);
        const l = await getDocumentLineage(id);
        setLineage(l);
      } catch (e) {
        console.error('获取文档血缘失败', e);
      } finally {
        if (!silent) setLineageLoading(false);
      }
    },
    [docPath, isShujiMarkdown]
  );

  useEffect(() => {
    setApprovalError('');
    setEditorError('');
    setDiffData(null);
    setDiffSource(null);
    setLineage(null);
    setViewMode(initialTab || 'content');
    contentRef.current = '';
    setContent('');

    loadDoc(false);
    loadDiff(false);
    loadLineage(false);
  }, [projectDir, docPath, loadDoc, loadDiff, loadLineage]);

  useEffect(() => {
    if (initialTab) setViewMode(initialTab);
  }, [initialTab]);

  useEffect(() => {
    if (!content || !isShujiMarkdown) return;
    const metaId = parsed.meta?.id;
    loadDiff(true, metaId);
    loadLineage(true, metaId);
  }, [content, isShujiMarkdown, parsed.meta?.id, loadDiff, loadLineage]);

  useEffect(() => {
    const events = ['chat-message', 'dept-log', 'plan-update'];
    const refresh = () => {
      loadDoc(true);
      loadDiff(true, parsed.meta?.id);
    };
    const unlistens = events.map((evt) => listen(evt, refresh));
    return () => {
      unlistens.forEach((p) => p.then((f) => f()));
    };
  }, [loadDoc, loadDiff, parsed.meta?.id]);

  const handleOpenInEditor = async () => {
    setOpeningEditor(true);
    setEditorError('');
    try {
      await openInExternalEditor(projectDir, docPath);
    } catch (e) {
      setEditorError(formatError(e));
    } finally {
      setOpeningEditor(false);
    }
  };

  const handleApproval = async () => {
    setApproving(true);
    setApprovalError('');
    try {
      const msg = `朕已御批。${comment ? ' ' + comment : ''}`;
      await apiSetStatus(docId, 'approved', comment || undefined);
      try {
        await sendMessage(msg);
      } catch (e) {
        setApprovalError(t('docPreview.approvalNotifyFailed', { error: formatError(e) }));
        await loadDoc(true);
        return;
      }
      await loadDoc(true);
    } catch (e) {
      setApprovalError(formatError(e));
    } finally {
      setApproving(false);
    }
  };

  if (loading && !contentRef.current) {
    return (
      <div className="doc-preview-shell h-full min-w-0 overflow-hidden bg-surface-paper flex flex-col">
        <div className="doc-preview-body flex-1 min-h-0 min-w-0 overflow-auto p-6 text-body text-ink-400">
          {t('docPreview.loading')}
        </div>
      </div>
    );
  }
  if (error) {
    return (
      <div className="doc-preview-shell h-full min-w-0 overflow-hidden bg-surface-paper flex flex-col">
        <div className="doc-preview-body flex-1 min-h-0 min-w-0 overflow-auto p-6 text-body text-vermillion">
          {error}
        </div>
      </div>
    );
  }

  const showDiffTab = diffData?.has_previous;
  const fileName = parts[parts.length - 1] || docPath;

  return (
    <div className="doc-preview-shell h-full min-w-0 overflow-hidden bg-surface-paper flex flex-col">
      <div className="doc-preview-toolbar shrink-0 min-w-0 border-b border-fold bg-surface-parchment/80">
        <div className="flex items-center gap-2 px-3 py-2 min-w-0 border-b border-fold/60">
          <span className="text-ui font-mono text-ink-800 truncate shrink-0 max-w-[40%]">
            {fileName}
          </span>
          <div className="text-[10px] text-ink-400 font-mono flex flex-wrap gap-0.5 flex-1 min-w-0 truncate">
            {parts.map((p, i) => (
              <span key={`${p}-${i}`} className="truncate">
                {i > 0 && <span className="mx-0.5 text-ink-300">/</span>}
                {p}
              </span>
            ))}
          </div>
          <button
            type="button"
            onClick={handleOpenInEditor}
            disabled={openingEditor}
            title={openInEditorText}
            className="shrink-0 text-ui font-medium px-2.5 py-1 rounded border border-fold text-ink-600 hover:text-ink-900 hover:bg-ink-100 transition disabled:opacity-50"
          >
            {openingEditor ? t('common.loading') : openInEditorText}
          </button>
        </div>
        {editorError && (
          <p className="px-3 py-1.5 text-caption text-vermillion border-b border-fold/60">
            {editorError}
          </p>
        )}
        <div className="flex gap-0.5 px-2 min-w-0 overflow-x-auto">
          <button
            onClick={() => setViewMode('content')}
            className={`px-3 py-1.5 text-ui font-medium whitespace-nowrap transition border-b-2 -mb-px ${
              viewMode === 'content'
                ? 'border-vermillion text-ink-900'
                : 'border-transparent text-ink-400 hover:text-ink-600'
            }`}
          >
            {t('document.fullText')}
          </button>
          {showDiffTab && (
            <button
              onClick={() => setViewMode('diff')}
              className={`px-3 py-1.5 text-ui font-medium whitespace-nowrap transition border-b-2 -mb-px ${
                viewMode === 'diff'
                  ? 'border-vermillion text-ink-900'
                  : 'border-transparent text-ink-400 hover:text-ink-600'
              }`}
            >
              {t('document.diff')}
              {diffSource === 'audit' && (
                <span className="ml-1 text-[10px] font-normal text-ink-400">
                  {t('docPreview.auditDiff')}
                </span>
              )}
              <span className="ml-1 text-caption text-ink-400">
                {diffData ? `+${diffData.added}/-${diffData.removed}` : ''}
              </span>
            </button>
          )}
          {isShujiMarkdown && (
            <button
              onClick={() => setViewMode('lineage')}
              className={`px-3 py-1.5 text-ui font-medium whitespace-nowrap transition border-b-2 -mb-px ${
                viewMode === 'lineage'
                  ? 'border-vermillion text-ink-900'
                  : 'border-transparent text-ink-400 hover:text-ink-600'
              }`}
            >
              {t('document.lineage')}
            </button>
          )}
        </div>
      </div>

      <div className="doc-preview-body flex-1 min-h-0 min-w-0 overflow-auto px-4 py-4 lg:px-6 lg:py-5">
        {docStatus === 'in_review' && (
          <div className="mb-4 rounded-lg border border-vermillion/40 bg-vermillion/5 px-3 py-3">
            <div className="flex items-center justify-between gap-3 flex-wrap">
              <div className="min-w-0">
                <h3 className="font-display text-sm font-bold text-ink-900">
                  {t('document.pendingApproval')}
                </h3>
                <p className="text-caption text-ink-600 mt-0.5">{t('document.approvalRequired')}</p>
              </div>
              <button
                onClick={handleApproval}
                disabled={approving}
                className="bg-jade hover:bg-jade/80 text-white text-ui font-bold px-3 py-1.5 rounded transition disabled:opacity-50 shrink-0"
              >
                {approving ? t('common.loading') : t('document.approve')}
              </button>
            </div>
            <div className="mt-2">
              <input
                type="text"
                placeholder={t('document.imperialNote')}
                value={comment}
                onChange={(e) => setComment(e.target.value)}
                className="w-full min-w-0 px-3 py-1.5 border border-fold rounded text-body bg-surface-paper"
              />
            </div>
            <p className="text-[11px] text-ink-400 mt-2 leading-relaxed">
              {t('approval.notSatisfiedHint')}
            </p>
            {approvalError && <p className="text-caption text-vermillion mt-1">{approvalError}</p>}
          </div>
        )}

        {viewMode === 'lineage' ? (
          lineageLoading ? (
            <div className="text-body text-ink-400">{t('docPreview.loadingLineage')}</div>
          ) : lineage ? (
            <LineageTree node={lineage} depth={0} />
          ) : (
            <div className="text-body text-ink-400 text-center">{t('docPreview.noLineage')}</div>
          )
        ) : viewMode === 'diff' ? (
          diffLoading && !diffData ? (
            <div className="text-body text-ink-400">{t('docPreview.loadingDiff')}</div>
          ) : diffData ? (
            <DiffView diff={diffData.diff} audit={diffSource === 'audit'} />
          ) : (
            <div className="text-body text-ink-400 text-center">{t('docPreview.noDiff')}</div>
          )
        ) : (
          <>
            {isShujiMarkdown && parsed.meta && <FrontmatterMetadata meta={parsed.meta} />}
            {isMarkdown ? (
              <article className="prose prose-shuji doc-preview-markdown max-w-none">
                <ReactMarkdown remarkPlugins={[remarkGfm]} rehypePlugins={[rehypeHighlight]}>
                  {(isShujiMarkdown ? parsed.body : content) || t('docPreview.fileEmpty')}
                </ReactMarkdown>
              </article>
            ) : (
              <CodePreview
                content={content}
                path={docPath}
                openLineLabel={openLineInEditorText}
                onOpenLine={async (line) => {
                  setEditorError('');
                  try {
                    await openInExternalEditor(projectDir, docPath, line);
                  } catch (e) {
                    setEditorError(formatError(e));
                  }
                }}
              />
            )}
          </>
        )}
      </div>
    </div>
  );
}

function DiffView({ diff, audit = false }: { diff: string; audit?: boolean }) {
  const { t } = useTranslation();
  if (!diff) {
    return <div className="p-6 text-body text-ink-400 text-center">{t('docPreview.noDiff')}</div>;
  }

  const lines = diff.split('\n');

  return (
    <div
      className="doc-preview-diff min-w-0 rounded-lg border overflow-hidden"
      style={{
        borderColor: 'var(--code-border)',
        backgroundColor: 'var(--code-bg)',
      }}
    >
      <div
        className="h-8 flex items-center px-3 text-[11px] font-mono shrink-0"
        style={{
          backgroundColor: 'var(--code-tab-bg)',
          borderBottom: '1px solid var(--code-border)',
          color: 'var(--code-muted)',
        }}
      >
        <span>{audit ? t('docPreview.auditDiffHeader') : 'Unified Diff'}</span>
      </div>
      <div className="doc-preview-diff-scroll overflow-auto min-w-0 text-[13px] leading-[22px] font-[Cascadia_Code,JetBrains_Mono,Consolas,Menlo,Monaco,monospace]">
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

function CodePreview({
  content,
  path,
  openLineLabel,
  onOpenLine,
}: {
  content: string;
  path: string;
  openLineLabel?: string;
  onOpenLine?: (line: number) => void | Promise<void>;
}) {
  const { t } = useTranslation();
  const lines = (content || t('docPreview.fileEmpty')).split(/\r?\n/);
  const language = languageName(path);
  const lineClickable = Boolean(onOpenLine);

  return (
    <div
      className="doc-preview-code min-w-0 rounded-lg border overflow-hidden"
      style={{
        borderColor: 'var(--code-border)',
        backgroundColor: 'var(--code-bg)',
      }}
    >
      <div
        className="h-8 flex items-center justify-between text-[11px] shrink-0 min-w-0"
        style={{
          backgroundColor: 'var(--code-tab-bg)',
          borderBottom: '1px solid var(--code-border)',
        }}
      >
        <div
          className="h-full px-3 flex items-center gap-2 font-mono min-w-0"
          style={{
            backgroundColor: 'var(--code-bg)',
            borderRight: '1px solid var(--code-border)',
            color: 'var(--code-text)',
          }}
        >
          <span style={{ color: 'var(--code-muted)' }}>{fileGlyph(path)}</span>
          <span className="truncate">{basenameFromPath(path)}</span>
        </div>
        <div
          className="px-3 font-mono flex items-center gap-3 shrink-0"
          style={{ color: 'var(--code-muted)' }}
        >
          <span>{language}</span>
          <span>{lines.length.toLocaleString()} lines</span>
          <span>{content.length.toLocaleString()} chars</span>
        </div>
      </div>
      <div className="doc-preview-code-scroll overflow-auto min-w-0 text-[13px] leading-[22px] font-[Cascadia_Code,JetBrains_Mono,Consolas,Menlo,Monaco,monospace]">
        <table className="w-full border-separate border-spacing-0">
          <tbody>
            {lines.map((line, index) => (
              <tr key={index} className="code-preview-row">
                <td
                  className={`select-none sticky left-0 w-14 min-w-14 pr-3 text-right align-top ${
                    lineClickable ? 'cursor-pointer hover:text-vermillion hover:underline' : ''
                  }`}
                  style={{
                    backgroundColor: 'var(--code-bg)',
                    color: 'var(--code-line-num)',
                    borderRight: '1px solid var(--code-border)',
                  }}
                  title={lineClickable ? openLineLabel : undefined}
                  onClick={lineClickable ? () => onOpenLine?.(index + 1) : undefined}
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

function FrontmatterMetadata({ meta }: { meta: Record<string, string> }) {
  const { t } = useTranslation();
  const labels: Record<string, string> = {
    id: 'ID',
    type: t('document.type'),
    author: t('document.author'),
    timestamp: t('document.time'),
    refs: t('document.refs'),
    status: t('document.status'),
  };

  const summaryParts = ['id', 'type', 'status'].map((key) => meta[key]).filter(Boolean);

  return (
    <details className="doc-preview-metadata mb-4 border-b border-fold pb-3 min-w-0">
      <summary className="text-caption font-mono text-ink-500 cursor-pointer select-none list-none [&::-webkit-details-marker]:hidden">
        <span className="text-ink-400">{t('docPreview.metadata')}</span>
        {summaryParts.length > 0 && (
          <span className="ml-2 text-ink-600">{summaryParts.join(' · ')}</span>
        )}
      </summary>
      <dl className="mt-2 grid grid-cols-1 sm:grid-cols-2 gap-x-4 gap-y-1 text-ui font-mono">
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
            <div key={key} className="flex min-w-0 gap-2">
              <dt className="w-16 shrink-0 text-ink-400">{labels[key] || key}</dt>
              <dd className={`break-all min-w-0 ${statusColor}`}>{value}</dd>
            </div>
          );
        })}
      </dl>
    </details>
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
