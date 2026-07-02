import type { ViewMode } from './useDocPreviewData';
import type { DocumentDiff } from '../../api';

export default function DocPreviewToolbar({
  fileName,
  parts,
  viewMode,
  setViewMode,
  showDiffTab,
  diffData,
  diffSource,
  isShujiMarkdown,
  openInEditorText,
  openingEditor,
  editorError,
  onOpenInEditor,
  t,
}: {
  fileName: string;
  parts: string[];
  viewMode: ViewMode;
  setViewMode: (v: ViewMode) => void;
  showDiffTab: boolean;
  diffData: DocumentDiff | null;
  diffSource: 'audit' | 'git' | null;
  isShujiMarkdown: boolean;
  openInEditorText: string;
  openingEditor: boolean;
  editorError: string;
  onOpenInEditor: () => void;
  t: (key: string) => string;
}) {
  return (
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
          onClick={onOpenInEditor}
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
          className={`px-3 py-1.5 text-ui font-medium whitespace-nowrap transition border-b-2 -mb-px ${viewMode === 'content' ? 'border-vermillion text-ink-900' : 'border-transparent text-ink-400 hover:text-ink-600'}`}
        >
          {t('document.fullText')}
        </button>
        {showDiffTab && (
          <button
            onClick={() => setViewMode('diff')}
            className={`px-3 py-1.5 text-ui font-medium whitespace-nowrap transition border-b-2 -mb-px ${viewMode === 'diff' ? 'border-vermillion text-ink-900' : 'border-transparent text-ink-400 hover:text-ink-600'}`}
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
            className={`px-3 py-1.5 text-ui font-medium whitespace-nowrap transition border-b-2 -mb-px ${viewMode === 'lineage' ? 'border-vermillion text-ink-900' : 'border-transparent text-ink-400 hover:text-ink-600'}`}
          >
            {t('document.lineage')}
          </button>
        )}
      </div>
    </div>
  );
}
