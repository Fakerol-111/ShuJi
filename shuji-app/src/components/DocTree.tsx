import { useEffect, useState, useCallback, useRef } from 'react';
import { useTranslation } from 'react-i18next';
import { listen } from '@tauri-apps/api/event';
import { listShujiTree } from '../api';
import { formatError } from '../utils/error';
import type { ShujiEntry } from '../api';

interface DocTreeProps {
  projectDir: string;
  selectedDoc: string | null;
  onSelect: (path: string) => void;
}

export default function DocTree({ projectDir, selectedDoc, onSelect }: DocTreeProps) {
  const { t } = useTranslation();
  const [tree, setTree] = useState<ShujiEntry[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState('');
  const refreshTimer = useRef<ReturnType<typeof setTimeout> | null>(null);
  // Persist expanded state across tree refreshes so user-closed folders stay closed
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

  // Debounced refresh — coalesces rapid events into one reload
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

  if (error) return <div className="p-3 text-ui text-vermillion">{error}</div>;
  if (loading && tree.length === 0)
    return (
      <div className="p-3 space-y-2">
        <LoadingSkeleton />
        <LoadingSkeleton />
      </div>
    );
  if (tree.length === 0 && !loading)
    return <div className="p-3 text-ui text-ink-400">{t('docTree.noPreviewFiles')}</div>;

  return (
    <div className="py-2 text-ui">
      <div className="sticky top-0 z-10 bg-surface-parchment px-2 pb-2 flex justify-end items-center gap-2 border-b border-fold mb-1">
        {loading && <span className="text-[10px] text-ink-400 animate-pulse">{t('docTree.refreshing')}</span>}
        <button
          onClick={loadTree}
          className="px-2 py-1 rounded text-ui text-ink-500 hover:bg-ink-100 hover:text-ink-800"
        >
          {t('common.refresh')}
        </button>
      </div>
      {tree.map((entry) => (
        <DocNode
          key={entry.path}
          entry={entry}
          selectedDoc={selectedDoc}
          onSelect={onSelect}
          depth={0}
          expandedRef={expandedRef}
        />
      ))}
    </div>
  );
}

function DocNode({
  entry,
  selectedDoc,
  onSelect,
  depth,
  expandedRef,
}: {
  entry: ShujiEntry;
  selectedDoc: string | null;
  onSelect: (path: string) => void;
  depth: number;
  expandedRef: React.MutableRefObject<Record<string, boolean>>;
}) {
  const initialState = expandedRef.current[entry.path] ?? true;
  const [open, setOpen] = useState(initialState);
  const active = selectedDoc === entry.path;

  // Sync toggle changes back to ref (survives remounts from loading state)
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
              depth={depth + 1}
              expandedRef={expandedRef}
            />
          ))}
      </div>
    );
  }

  return (
    <button
      onClick={() => onSelect(entry.path)}
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
      <span className="ml-auto text-caption text-ink-400 shrink-0">{entry.type_label}</span>
    </button>
  );
}

function fileIcon(name: string) {
  if (name.endsWith('.md')) return '◇';
  if (/\.(ts|tsx|js|jsx|rs|py|css|html)$/.test(name)) return '<>';
  if (/\.(json|toml|ya?ml)$/.test(name)) return '{}';
  return '·';
}

/** Skeleton placeholder for loading state */
function LoadingSkeleton() {
  return (
    <div className="flex items-center gap-2 px-3 py-2 animate-pulse">
      <div className="w-3 h-3 rounded bg-ink-200" />
      <div className="h-3 flex-1 rounded bg-ink-200" />
    </div>
  );
}
