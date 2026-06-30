import { useEffect, useState, useCallback, useRef, useMemo } from 'react';
import { useTranslation } from 'react-i18next';
import type { TFunction } from 'i18next';
import { listen } from '@tauri-apps/api/event';
import { listShujiTree, openInExternalEditor, openProjectInExternalEditor } from '../api';
import { formatError } from '../utils/error';
import { useEditorConfig } from '../hooks/useEditorConfig';
import { openInEditorLabel, openProjectInEditorLabel } from '../utils/editorLabel';
import type { ShujiEntry } from '../api';

interface DocTreeProps {
  projectDir: string;
  selectedDoc: string | null;
  onSelect: (path: string) => void;
}

interface ContextMenuState {
  x: number;
  y: number;
  path: string;
}

function filterShujiOnly(entries: ShujiEntry[]): ShujiEntry[] {
  return entries
    .map((entry) => {
      if (entry.path === '.shuji' || entry.path.startsWith('.shuji/')) return entry;
      if (!entry.is_dir) return null;
      const children = filterShujiOnly(entry.children);
      return children.length > 0 ? { ...entry, children } : null;
    })
    .filter(Boolean) as ShujiEntry[];
}

function formatTypeLabel(name: string, t: TFunction): string {
  const base = name.replace(/\.md$/, '');
  const prefix = base.split(/[-_]/)[0]?.toLowerCase();
  const shujiTypes: Record<string, string> = {
    reqs: t('docTree.typeReqs'),
    dsgn: t('docTree.typeDsgn'),
    plan: t('docTree.typePlan'),
    pdsg: t('docTree.typePdsg'),
    ddtl: t('docTree.typeDdtl'),
    revw: t('docTree.typeRevw'),
    task: t('docTree.typeTask'),
    ctrt: t('docTree.typeCtrt'),
    rprt: t('docTree.typeRprt'),
    anls: t('docTree.typeAnls'),
  };
  if (prefix && shujiTypes[prefix]) return shujiTypes[prefix];

  const ext = name.split('.').pop()?.toLowerCase();
  const extLabels: Record<string, string> = {
    md: 'MD',
    ts: 'TS',
    tsx: 'TSX',
    js: 'JS',
    jsx: 'JSX',
    rs: 'RS',
    py: 'PY',
    json: 'JSON',
    jsonl: 'JSONL',
    toml: 'TOML',
    yaml: 'YAML',
    yml: 'YAML',
    css: 'CSS',
    html: 'HTML',
  };
  return ext ? extLabels[ext] || ext.toUpperCase() : '';
}

export default function DocTree({ projectDir, selectedDoc, onSelect }: DocTreeProps) {
  const { t } = useTranslation();
  const editorConfig = useEditorConfig();
  const openInEditorText = useMemo(() => openInEditorLabel(editorConfig, t), [editorConfig, t]);
  const openProjectInEditorText = useMemo(
    () => openProjectInEditorLabel(editorConfig, t),
    [editorConfig, t]
  );
  const [tree, setTree] = useState<ShujiEntry[]>([]);
  const [showAllFiles, setShowAllFiles] = useState(false);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState('');
  const [editorError, setEditorError] = useState('');
  const [openingProject, setOpeningProject] = useState(false);
  const [contextMenu, setContextMenu] = useState<ContextMenuState | null>(null);
  const refreshTimer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const expandedRef = useRef<Record<string, boolean>>({});

  const loadTree = useCallback(() => {
    if (!projectDir) return;
    setLoading(true);
    setError('');
    listShujiTree(projectDir)
      .then(setTree)
      .catch((e) => setError(formatError(e)))
      .finally(() => setLoading(false));
  }, [projectDir]);

  const debouncedRefresh = useCallback(() => {
    if (refreshTimer.current) clearTimeout(refreshTimer.current);
    refreshTimer.current = setTimeout(() => loadTree(), 400);
  }, [loadTree]);

  useEffect(() => {
    loadTree();
  }, [loadTree]);

  useEffect(() => {
    const events = ['chat-message', 'dept-log', 'plan-update'];
    const unlistens = events.map((evt) => listen(evt, debouncedRefresh));
    return () => {
      unlistens.forEach((p) => p.then((f) => f()));
    };
  }, [debouncedRefresh]);

  useEffect(() => {
    if (!contextMenu) return;
    const close = () => setContextMenu(null);
    window.addEventListener('click', close);
    window.addEventListener('scroll', close, true);
    return () => {
      window.removeEventListener('click', close);
      window.removeEventListener('scroll', close, true);
    };
  }, [contextMenu]);

  const handleOpenInEditor = async (relPath: string) => {
    setContextMenu(null);
    setEditorError('');
    try {
      await openInExternalEditor(projectDir, relPath);
    } catch (e) {
      setEditorError(formatError(e));
    }
  };

  const handleOpenProject = async () => {
    setOpeningProject(true);
    setEditorError('');
    try {
      await openProjectInExternalEditor(projectDir);
    } catch (e) {
      setEditorError(formatError(e));
    } finally {
      setOpeningProject(false);
    }
  };

  const displayTree = useMemo(
    () => (showAllFiles ? tree : filterShujiOnly(tree)),
    [tree, showAllFiles]
  );

  if (error) return <div className="p-3 text-ui text-vermillion">{error}</div>;
  if (loading && tree.length === 0)
    return (
      <div className="p-3 space-y-2">
        <LoadingSkeleton />
        <LoadingSkeleton />
      </div>
    );

  return (
    <div className="py-2 text-ui relative">
      <div className="sticky top-0 z-10 bg-surface-parchment px-2 pb-2 flex justify-between items-center gap-2 border-b border-fold mb-1">
        <div className="flex rounded border border-fold overflow-hidden shrink-0">
          <button
            onClick={() => setShowAllFiles(false)}
            className={`px-2 py-0.5 text-[10px] transition-colors ${
              !showAllFiles
                ? 'bg-vermillion/10 text-vermillion font-medium'
                : 'text-ink-500 hover:bg-ink-100'
            }`}
          >
            {t('docTree.shujiDocs')}
          </button>
          <button
            onClick={() => setShowAllFiles(true)}
            className={`px-2 py-0.5 text-[10px] border-l border-fold transition-colors ${
              showAllFiles
                ? 'bg-vermillion/10 text-vermillion font-medium'
                : 'text-ink-500 hover:bg-ink-100'
            }`}
          >
            {t('docTree.allFiles')}
          </button>
        </div>
        <div className="flex items-center gap-1 shrink-0">
          {loading && (
            <span className="text-[10px] text-ink-400 animate-pulse">
              {t('docTree.refreshing')}
            </span>
          )}
          <button
            type="button"
            onClick={handleOpenProject}
            disabled={openingProject}
            title={openProjectInEditorText}
            className="px-2 py-1 rounded text-[10px] text-ink-500 hover:bg-ink-100 hover:text-ink-800 disabled:opacity-50 whitespace-nowrap"
          >
            {openingProject ? t('common.loading') : openProjectInEditorText}
          </button>
          <button
            onClick={loadTree}
            className="px-2 py-1 rounded text-ui text-ink-500 hover:bg-ink-100 hover:text-ink-800"
          >
            {t('common.refresh')}
          </button>
        </div>
      </div>

      {editorError && (
        <p className="mx-2 mb-2 px-2 py-1.5 text-caption text-vermillion bg-vermillion/5 rounded border border-vermillion/20">
          {editorError}
        </p>
      )}

      {displayTree.length === 0 && !loading ? (
        <div className="p-3 text-center text-ink-400">
          <p className="text-ui">
            {showAllFiles ? t('docTree.noPreviewFiles') : t('docTree.noShujiDocs')}
          </p>
          {!showAllFiles && (
            <button
              onClick={() => setShowAllFiles(true)}
              className="mt-2 text-ui text-vermillion hover:underline"
            >
              {t('docTree.viewAllFiles')}
            </button>
          )}
        </div>
      ) : (
        displayTree.map((entry) => (
          <DocNode
            key={entry.path}
            entry={entry}
            selectedDoc={selectedDoc}
            onSelect={onSelect}
            onContextMenu={(path, x, y) => setContextMenu({ path, x, y })}
            depth={0}
            expandedRef={expandedRef}
          />
        ))
      )}

      {contextMenu && (
        <div
          className="fixed z-50 min-w-[160px] rounded-lg border border-fold bg-surface-paper shadow-lg py-1 text-ui"
          style={{ left: contextMenu.x, top: contextMenu.y }}
          onClick={(e) => e.stopPropagation()}
        >
          <button
            type="button"
            className="w-full px-3 py-1.5 text-left hover:bg-ink-100 text-ink-800"
            onClick={() => handleOpenInEditor(contextMenu.path)}
          >
            {openInEditorText}
          </button>
        </div>
      )}
    </div>
  );
}

function DocNode({
  entry,
  selectedDoc,
  onSelect,
  onContextMenu,
  depth,
  expandedRef,
}: {
  entry: ShujiEntry;
  selectedDoc: string | null;
  onSelect: (path: string) => void;
  onContextMenu: (path: string, x: number, y: number) => void;
  depth: number;
  expandedRef: React.MutableRefObject<Record<string, boolean>>;
}) {
  const { t } = useTranslation();
  const initialState = expandedRef.current[entry.path] ?? true;
  const [open, setOpen] = useState(initialState);
  const active = selectedDoc === entry.path;

  const handleToggle = () => {
    const next = !open;
    expandedRef.current[entry.path] = next;
    setOpen(next);
  };

  if (entry.is_dir) {
    return (
      <div>
        <button
          onClick={handleToggle}
          className="w-full flex items-center gap-1 px-2 py-1 text-left text-ink-500 hover:text-ink-800 hover:bg-ink-100"
          style={{ paddingLeft: 8 + depth * 12 }}
        >
          <span className="w-3 text-caption">{open ? '▾' : '▸'}</span>
          <span className="truncate font-medium">{entry.name}</span>
          <span className="ml-auto text-caption text-ink-400">{entry.children.length}</span>
        </button>
        {open &&
          entry.children.map((child) => (
            <DocNode
              key={child.path}
              entry={child}
              selectedDoc={selectedDoc}
              onSelect={onSelect}
              onContextMenu={onContextMenu}
              depth={depth + 1}
              expandedRef={expandedRef}
            />
          ))}
      </div>
    );
  }

  const typeLabel = formatTypeLabel(entry.name, t);

  return (
    <button
      onClick={() => onSelect(entry.path)}
      onContextMenu={(e) => {
        e.preventDefault();
        onContextMenu(entry.path, e.clientX, e.clientY);
      }}
      className={`w-full flex items-center gap-1 px-2 py-1 text-left transition-colors ${
        active
          ? 'bg-vermillion/10 text-vermillion border-r-2 border-vermillion'
          : 'text-ink-600 hover:bg-ink-100 hover:text-ink-900'
      }`}
      style={{ paddingLeft: 12 + depth * 12 }}
      title={entry.path}
    >
      <span className="text-caption">{fileIcon(entry.name)}</span>
      <span className="truncate font-mono text-ui">{entry.name}</span>
      {typeLabel && <span className="ml-auto text-caption text-ink-400 shrink-0">{typeLabel}</span>}
    </button>
  );
}

function fileIcon(name: string) {
  if (name.endsWith('.md')) return '◇';
  if (/\.(ts|tsx|js|jsx|rs|py|css|html)$/.test(name)) return '<>';
  if (/\.(json|toml|ya?ml)$/.test(name)) return '{}';
  return '·';
}

function LoadingSkeleton() {
  return (
    <div className="flex items-center gap-2 px-3 py-2 animate-pulse">
      <div className="w-3 h-3 rounded bg-ink-200" />
      <div className="h-3 flex-1 rounded bg-ink-200" />
    </div>
  );
}
