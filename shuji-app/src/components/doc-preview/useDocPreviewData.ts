import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { listen } from '@tauri-apps/api/event';
import { readShujiDoc, getDocumentDiff } from '../../api';
import { formatError } from '../../utils/error';
import { parseFrontmatter, docIdFromPath } from './frontmatter';
import { loadAuditDiff } from './diff';
import type { DocumentDiff } from '../../api';
import type { LineageNode } from '../../types';
import { getDocumentLineage } from '../../api';

export type ViewMode = 'content' | 'diff' | 'lineage';

export interface DocPreviewState {
  content: string;
  loading: boolean;
  error: string;
  approving: boolean;
  approvalError: string;
  comment: string;
  viewMode: ViewMode;
  diffData: DocumentDiff | null;
  diffLoading: boolean;
  diffSource: 'audit' | 'git' | null;
  lineage: LineageNode | null;
  lineageLoading: boolean;
  editorError: string;
  openingEditor: boolean;
  contentRef: React.MutableRefObject<string>;
  parsed: { meta: Record<string, string> | null; body: string };
  docId: string;
  docStatus: string;
}

export function useDocPreviewData(projectDir: string, docPath: string, initialTab?: ViewMode) {
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
  const parsed = useMemo(() => parseFrontmatter(content), [content]);
  const docId = isShujiMarkdown ? docIdFromPath(docPath, parsed.meta?.id) : '';

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

  const docStatus = parsed.meta?.status || '';

  return {
    content,
    loading,
    error,
    setError,
    approving,
    setApproving,
    approvalError,
    setApprovalError,
    comment,
    setComment,
    viewMode,
    setViewMode,
    diffData,
    diffLoading,
    setDiffLoading,
    diffSource,
    lineage,
    lineageLoading,
    editorError,
    setEditorError,
    openingEditor,
    setOpeningEditor,
    contentRef,
    isShujiMarkdown,
    parsed,
    docId,
    docStatus,
    loadDoc,
    loadDiff,
    loadLineage,
  };
}
