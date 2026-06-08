import { useEffect, useState } from "react";
import { readDocumentDiff } from "../../api";
import type { LineageNode, ChainNode } from "../../types";

// ── DiffViewer ───────────────────────────────────────────────

export function DiffViewer({ filename }: { filename: string }) {
  const [patch, setPatch] = useState<string | null>(null);
  const [open, setOpen] = useState(false);
  const [loading, setLoading] = useState(false);

  useEffect(() => {
    if (open && !patch && !loading) {
      setLoading(true);
      readDocumentDiff(filename)
        .then(setPatch)
        .catch(() => setPatch("(加载失败)"))
        .finally(() => setLoading(false));
    }
  }, [open, filename, patch, loading]);

  return (
    <div className="mt-0.5">
      <button
        onClick={() => setOpen(!open)}
        className="text-[10px] text-ink-400 hover:text-ink-600 underline"
      >
        {open ? "收起 diff" : "查看 diff"}
      </button>
      {open && (
        <pre className="mt-1 p-1.5 rounded bg-ink-100 text-[10px] font-mono leading-tight overflow-x-auto max-h-40 overflow-y-auto whitespace-pre">
          {loading ? "载入中..." : patch || "(无内容)"}
        </pre>
      )}
    </div>
  );
}

// ── DocCard ──────────────────────────────────────────────────

const STAGE_LABELS: Record<string, string> = {
  reqs: "需求",
  design: "设计",
  plan: "计划",
  contract: "契约",
  other: "其他",
};
const STAGE_COLORS: Record<string, string> = {
  reqs: "text-jade bg-jade/10",
  design: "text-azure bg-azure/10",
  plan: "text-gold bg-gold/10",
  contract: "text-ink-600 bg-ink-100",
  other: "text-ink-400 bg-ink-100",
};

export function docIdToPath(docId: string): string {
  const prefix = docId.split("_")[0];
  const dirMap: Record<string, string> = {
    dsgn: "designs",
    plan: "designs",
    pdsg: "designs",
    ddtl: "designs/detail",
    revw: "reviews",
    task: "tasks",
    ctrt: "contracts",
    rprt: "reports",
    anls: "analysis",
    reqs: "requirements",
  };
  const dir = dirMap[prefix] || "";
  return `.shuji/${dir ? dir + "/" : ""}${docId}.md`;
}

export function DocCard({
  node,
  onDocSelect,
}: {
  node: ChainNode;
  onDocSelect?: (path: string) => void;
}) {
  const colorClass = STAGE_COLORS[node.stage] || "text-ink-400 bg-ink-100";
  return (
    <div
      className="rounded border border-fold p-2 hover:bg-ink-100/30 transition-colors cursor-pointer"
      onClick={() => onDocSelect?.(docIdToPath(node.id))}
    >
      <div className="flex items-center gap-1.5 text-caption">
        <span className={`px-1 rounded text-[10px] font-mono ${colorClass}`}>
          {STAGE_LABELS[node.stage] || node.doc_type}
        </span>
        <span className="text-ink-700 font-mono text-[11px]">{node.id}</span>
        <span className="text-ink-400">{node.author}</span>
      </div>
      <div className="text-caption text-ink-400 truncate mt-0.5">
        {node.content_preview}
      </div>
      <div className="text-[9px] text-ink-300 font-mono mt-0.5">
        {node.timestamp}
      </div>
    </div>
  );
}

// ── LineageTree ──────────────────────────────────────────────

export function LineageTree({
  node,
  depth,
}: {
  node: LineageNode;
  depth: number;
}) {
  return (
    <div className="ml-3 border-l border-fold pl-2">
      <div className="flex items-center gap-1.5 text-caption py-1">
        <span className="text-ink-500 font-mono text-[10px]">{node.id}</span>
        <span className="text-ink-400">({node.doc_type})</span>
        <span className="text-ink-300">{node.author}</span>
        {node.status && (
          <span
            className={`px-1 rounded text-[9px] ${node.status === "approved" ? "bg-jade/10 text-jade" : "bg-gold/10 text-gold"}`}
          >
            {node.status}
          </span>
        )}
      </div>
      {node.children.map((child) => (
        <LineageTree key={child.id} node={child} depth={depth + 1} />
      ))}
    </div>
  );
}
