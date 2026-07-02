import { useMemo } from 'react';
import type { ViewMode } from './doc-preview/useDocPreviewData';
import { useTranslation } from 'react-i18next';
import ReactMarkdown from 'react-markdown';
import remarkGfm from 'remark-gfm';
import rehypeHighlight from 'rehype-highlight';
import { setDocumentStatus as apiSetStatus, sendMessage, openInExternalEditor } from '../api';
import { formatError } from '../utils/error';
import { useEditorConfig } from '../hooks/useEditorConfig';
import { openInEditorLabel, openLineInEditorLabel } from '../utils/editorLabel';
import { splitPathParts } from '../utils/pathBasename';
import { useDocPreviewData } from './doc-preview/useDocPreviewData';
import DocPreviewToolbar from './doc-preview/DocPreviewToolbar';
import ApprovalBanner from './doc-preview/ApprovalBanner';
import DiffView from './doc-preview/DiffView';
import CodePreview from './doc-preview/CodePreview';
import FrontmatterMetadata from './doc-preview/FrontmatterMetadata';
import LineageView from './doc-preview/LineageView';

interface DocPreviewProps {
  projectDir: string;
  docPath: string;
  initialTab?: ViewMode;
}

export default function DocPreview({ projectDir, docPath, initialTab }: DocPreviewProps) {
  const { t } = useTranslation();
  const editorConfig = useEditorConfig();
  const openInEditorText = useMemo(() => openInEditorLabel(editorConfig, t), [editorConfig, t]);
  const openLineInEditorText = useMemo(
    () => openLineInEditorLabel(editorConfig, t),
    [editorConfig, t]
  );

  const {
    content,
    loading,
    error,
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
  } = useDocPreviewData(projectDir, docPath, initialTab);

  const isMarkdown = docPath.endsWith('.md');
  const parts = splitPathParts(docPath);

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
      await apiSetStatus(docId, 'approved', comment || undefined);
      try {
        await sendMessage(`朕已御批。${comment ? ' ' + comment : ''}`);
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
      <DocPreviewToolbar
        t={t}
        fileName={fileName}
        parts={parts}
        viewMode={viewMode}
        setViewMode={setViewMode}
        showDiffTab={!!showDiffTab}
        diffData={diffData}
        diffSource={diffSource}
        isShujiMarkdown={isShujiMarkdown}
        openInEditorText={openInEditorText}
        openingEditor={openingEditor}
        editorError={editorError}
        onOpenInEditor={handleOpenInEditor}
      />

      <div className="doc-preview-body flex-1 min-h-0 min-w-0 overflow-auto px-4 py-4 lg:px-6 lg:py-5">
        {docStatus === 'in_review' && (
          <ApprovalBanner
            approving={approving}
            approvalError={approvalError}
            comment={comment}
            onCommentChange={setComment}
            onApprove={handleApproval}
          />
        )}

        {viewMode === 'lineage' ? (
          lineageLoading ? (
            <div className="text-body text-ink-400">{t('docPreview.loadingLineage')}</div>
          ) : lineage ? (
            <LineageView node={lineage} depth={0} />
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
