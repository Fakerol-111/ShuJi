import { useEffect, useState, useCallback, useRef } from "react";
import { listen } from "@tauri-apps/api/event";
import { listShujiTree } from "../api";
import type { ShujiEntry } from "../api";

interface DocTreeProps {
  projectDir: string;
  selectedDoc: string | null;
  onSelect: (path: string) => void;
}

export default function DocTree({ projectDir, selectedDoc, onSelect }: DocTreeProps) {
  const [tree, setTree] = useState<ShujiEntry[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState("");
  const refreshTimer = useRef<ReturnType<typeof setTimeout> | null>(null);

  const loadTree = useCallback(() => {
    if (!projectDir) return;
    setLoading(true);
    setError("");
    listShujiTree(projectDir)
      .then(setTree)
      .catch((e) => setError(String(e)))
      .finally(() => setLoading(false));
  }, [projectDir]);

  // Debounced refresh — coalesces rapid events into one reload
  const debouncedRefresh = useCallback(() => {
    if (refreshTimer.current) clearTimeout(refreshTimer.current);
    refreshTimer.current = setTimeout(() => loadTree(), 400);
  }, [loadTree]);

  useEffect(() => { loadTree(); }, [loadTree]);

  useEffect(() => {
    const unlisten1 = listen("chat-message", debouncedRefresh);
    const unlisten2 = listen("dept-log", debouncedRefresh);
    const unlisten3 = listen("plan-update", debouncedRefresh);
    return () => {
      unlisten1.then((f) => f());
      unlisten2.then((f) => f());
      unlisten3.then((f) => f());
    };
  }, [debouncedRefresh]);

  if (loading) return <div className="p-3 text-ui text-ink-400">开卷中…</div>;
  if (error) return <div className="p-3 text-ui text-vermillion">{error}</div>;
  if (tree.length === 0) return <div className="p-3 text-ui text-ink-400">暂无可预览文件</div>;

  return (
    <div className="py-2 text-ui">
      <div className="sticky top-0 z-10 bg-surface-parchment px-2 pb-2 flex justify-end border-b border-fold mb-1">
        <button onClick={loadTree} className="px-2 py-1 rounded text-ui text-ink-500 hover:bg-ink-100 hover:text-ink-800">
          刷新
        </button>
      </div>
      {tree.map((entry) => (
        <DocNode key={entry.path} entry={entry} selectedDoc={selectedDoc} onSelect={onSelect} depth={0} />
      ))}
    </div>
  );
}

function DocNode({ entry, selectedDoc, onSelect, depth }: { entry: ShujiEntry; selectedDoc: string | null; onSelect: (path: string) => void; depth: number }) {
  const [open, setOpen] = useState(true);
  const active = selectedDoc === entry.path;

  if (entry.is_dir) {
    return (
      <div>
        <button
          onClick={() => setOpen(!open)}
          className="w-full flex items-center gap-1 px-2 py-1 text-left text-ink-500 hover:text-ink-800 hover:bg-ink-100"
          style={{ paddingLeft: 8 + depth * 12 }}
        >
          <span className="w-3 text-caption">{open ? "▾" : "▸"}</span>
          <span className="truncate font-medium">{entry.name}</span>
          <span className="ml-auto text-caption text-ink-400">{entry.children.length}</span>
        </button>
        {open && entry.children.map((child) => (
          <DocNode key={child.path} entry={child} selectedDoc={selectedDoc} onSelect={onSelect} depth={depth + 1} />
        ))}
      </div>
    );
  }

  return (
    <button
      onClick={() => onSelect(entry.path)}
      className={`w-full flex items-center gap-1 px-2 py-1 text-left transition-colors ${
        active ? "bg-vermillion/10 text-vermillion border-r-2 border-vermillion" : "text-ink-600 hover:bg-ink-100 hover:text-ink-900"
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
  if (name.endsWith(".md")) return "◇";
  if (/\.(ts|tsx|js|jsx|rs|py|css|html)$/.test(name)) return "<>";
  if (/\.(json|toml|ya?ml)$/.test(name)) return "{}";
  return "·";
}
