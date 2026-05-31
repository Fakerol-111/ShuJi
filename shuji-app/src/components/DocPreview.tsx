import { useEffect, useMemo, useState } from "react";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import { readShujiDoc, setDocumentStatus as apiSetStatus, sendMessage } from "../api";
import { Card } from "./ui/Card";

interface DocPreviewProps {
  projectDir: string;
  docPath: string;
}

export default function DocPreview({ projectDir, docPath }: DocPreviewProps) {
  const [content, setContent] = useState("");
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState("");
  const [approving, setApproving] = useState(false);
  const [approvalError, setApprovalError] = useState("");
  const [comment, setComment] = useState("");

  useEffect(() => {
    setLoading(true);
    setError("");
    setApprovalError("");
    readShujiDoc(projectDir, docPath)
      .then((doc) => setContent(doc.content))
      .catch((e) => setError(String(e)))
      .finally(() => setLoading(false));
  }, [projectDir, docPath]);

  const isShujiMarkdown = docPath.startsWith(".shuji/") && docPath.endsWith(".md");
  const isMarkdown = docPath.endsWith(".md");
  const parsed = useMemo(() => parseFrontmatter(content), [content]);
  const parts = docPath.split("/");
  const docId = isShujiMarkdown && parsed.meta?.id || docPath.split("/").pop()?.replace(/\.md$/, "") || "";
  const docStatus = parsed.meta?.status || "";

  const handleApproval = async (status: "approved" | "rejected") => {
    setApproving(true);
    setApprovalError("");
    try {
      const msg = status === "approved" ? `朕已御批。${comment ? " " + comment : ""}` : `驳回。${comment ? " " + comment : ""}`;
      await apiSetStatus(docId, status, comment || undefined);
      await sendMessage(msg);
      // Re-fetch to update banner status
      const doc = await readShujiDoc(projectDir, docPath);
      setContent(doc.content);
    } catch (e) {
      setApprovalError(String(e));
    } finally {
      setApproving(false);
    }
  };

  if (loading) return <div className="p-6 text-body text-ink-400">开卷中…</div>;
  if (error) return <div className="p-6 text-body text-vermillion">{error}</div>;

  return (
    <div className="h-full overflow-y-auto surface-paper">
      <div className="px-6 py-6 lg:px-8 lg:py-8">
        <div className="text-caption text-ink-400 font-mono mb-4 flex flex-wrap gap-1">
          {parts.map((p, i) => (
            <span key={`${p}-${i}`}>
              {i > 0 && <span className="mx-1 text-ink-300">/</span>}{p}
            </span>
          ))}
        </div>

        {/* ── "待陛下朱批" banner ── */}
        {docStatus === "in_review" && (
          <div className="mb-4 rounded-xl border border-vermillion/30 bg-surface-elevated p-4 shadow-sm">
            <div className="flex items-center justify-between">
              <div>
                <h3 className="font-display text-sm font-bold text-ink-900">待陛下朱批</h3>
                <p className="text-caption text-ink-600 mt-0.5">此文档需皇帝御批后方可继续执行</p>
              </div>
              <div className="flex gap-2">
                <button
                  onClick={() => handleApproval("approved")}
                  disabled={approving}
                  className="bg-jade hover:bg-jade/80 text-white text-ui font-bold px-4 py-2 rounded-lg transition disabled:opacity-50"
                >
                  {approving ? "处理中..." : "准奏"}
                </button>
                <button
                  onClick={() => handleApproval("rejected")}
                  disabled={approving}
                  className="bg-vermillion hover:bg-vermillion-dark text-white text-ui font-bold px-4 py-2 rounded-lg transition disabled:opacity-50"
                >
                  封还
                </button>
              </div>
            </div>
            <div className="mt-2">
              <input
                type="text"
                placeholder="御批备注（可选）..."
                value={comment}
                onChange={(e) => setComment(e.target.value)}
                className="w-full px-3 py-1.5 border border-fold rounded-lg text-body bg-surface-parchment"
              />
            </div>
            {approvalError && <p className="text-caption text-vermillion mt-1">{approvalError}</p>}
          </div>
        )}

        {isShujiMarkdown && parsed.meta && <FrontmatterCard meta={parsed.meta} />}

        {isMarkdown ? (
          <article className="prose prose-shuji max-w-none">
            <ReactMarkdown remarkPlugins={[remarkGfm]}>{(isShujiMarkdown ? parsed.body : content) || "_文件为空_"}</ReactMarkdown>
          </article>
        ) : (
          <CodePreview content={content} path={docPath} />
        )}
      </div>
    </div>
  );
}

function CodePreview({ content, path }: { content: string; path: string }) {
  const lines = (content || "文件为空").split(/\r?\n/);
  const language = languageName(path);

  return (
    <div className="rounded-xl border shadow-sm overflow-hidden" style={{ borderColor: "var(--code-border)", backgroundColor: "var(--code-bg)" }}>
      <div className="h-9 flex items-center justify-between text-[11px]" style={{ backgroundColor: "var(--code-tab-bg)", borderBottom: "1px solid var(--code-border)" }}>
        <div className="h-full px-3 flex items-center gap-2 font-mono" style={{ backgroundColor: "var(--code-bg)", borderRight: "1px solid var(--code-border)", color: "var(--code-text)" }}>
          <span style={{ color: "var(--code-muted)" }}>{fileGlyph(path)}</span>
          <span className="truncate max-w-[520px]">{path.split("/").pop()}</span>
        </div>
        <div className="px-3 font-mono flex items-center gap-3" style={{ color: "var(--code-muted)" }}>
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
                <td className="select-none sticky left-0 w-14 min-w-14 pr-3 text-right align-top" style={{ backgroundColor: "var(--code-bg)", color: "var(--code-line-num)", borderRight: "1px solid var(--code-border)" }}>
                  {index + 1}
                </td>
                <td className="pl-4 pr-6 whitespace-pre align-top" style={{ color: "var(--code-text)" }}>
                  {line || "\u00A0"}
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
  const ext = path.split(".").pop()?.toLowerCase();
  const map: Record<string, string> = {
    rs: "Rust",
    ts: "TypeScript",
    tsx: "TSX",
    js: "JavaScript",
    jsx: "JSX",
    json: "JSON",
    jsonl: "JSONL",
    toml: "TOML",
    yaml: "YAML",
    yml: "YAML",
    css: "CSS",
    html: "HTML",
    py: "Python",
    sh: "Shell",
    ps1: "PowerShell",
    svg: "SVG",
    txt: "Text",
    env: "Env",
  };
  return ext ? map[ext] || ext.toUpperCase() : "Text";
}

function fileGlyph(path: string) {
  const ext = path.split(".").pop()?.toLowerCase();
  if (["ts", "tsx", "js", "jsx"].includes(ext || "")) return "TS";
  if (ext === "rs") return "RS";
  if (["json", "jsonl"].includes(ext || "")) return "{}";
  if (["toml", "yaml", "yml", "env"].includes(ext || "")) return "⚙";
  if (ext === "py") return "PY";
  return "TXT";
}

function FrontmatterCard({ meta }: { meta: Record<string, string> }) {
  const labels: Record<string, string> = {
    id: "ID",
    type: "类型",
    author: "作者",
    timestamp: "时间",
    refs: "引用",
    status: "状态",
  };
  return (
    <Card variant="parchment" className="mb-5 border-l-vermillion border-l-[3px] p-4">
      <div className="font-display text-ui text-ink-600 font-semibold mb-2">票拟</div>
      <div className="grid grid-cols-1 sm:grid-cols-2 gap-2">
        {Object.entries(meta).map(([key, value]) => {
          const statusColor = key === "status" && value === "in_review" ? "text-vermillion font-bold" :
            key === "status" && value === "approved" ? "text-jade font-bold" :
            key === "status" && value === "rejected" ? "text-vermillion/60 font-bold" :
            "text-ink-700";
          if (key === "notes" && !value) return null;
          if (key === "status" && !value) return null;
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

function parseFrontmatter(raw: string): { meta: Record<string, string> | null; body: string } {
  const match = raw.match(/^---\r?\n([\s\S]*?)\r?\n---\r?\n?/);
  if (!match) return { meta: null, body: raw };
  const header = match[1];
  const body = raw.slice(match[0].length).trimStart();
  const meta: Record<string, string> = {};
  for (const line of header.split(/\r?\n/)) {
    const idx = line.indexOf(":");
    if (idx > 0) meta[line.slice(0, idx).trim()] = line.slice(idx + 1).trim();
  }
  return { meta, body };
}
