import { useEffect, useMemo, useState } from "react";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import { readShujiDoc, setDocumentStatus as apiSetStatus, sendMessage } from "../api";

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

  if (loading) return <div className="p-6 text-sm text-ink-400">加载文件...</div>;
  if (error) return <div className="p-6 text-sm text-vermillion">{error}</div>;

  return (
    <div className="h-full overflow-y-auto bg-ink-50">
      <div className="max-w-5xl mx-auto px-8 py-6">
        <div className="text-xs text-ink-400 font-mono mb-4 flex flex-wrap gap-1">
          {parts.map((p, i) => (
            <span key={`${p}-${i}`}>
              {i > 0 && <span className="mx-1 text-ink-300">/</span>}{p}
            </span>
          ))}
        </div>

        {/* ── "待陛下朱批" banner ── */}
        {docStatus === "in_review" && (
          <div className="mb-4 rounded-xl border border-amber-300 bg-amber-50 p-4 shadow-sm">
            <div className="flex items-center justify-between">
              <div>
                <h3 className="text-sm font-bold text-amber-900">待陛下朱批</h3>
                <p className="text-xs text-amber-700 mt-0.5">此文档需皇帝御批后方可继续执行</p>
              </div>
              <div className="flex gap-2">
                <button
                  onClick={() => handleApproval("approved")}
                  disabled={approving}
                  className="bg-green-600 hover:bg-green-700 text-white text-xs font-bold px-4 py-2 rounded-lg transition disabled:opacity-50"
                >
                  {approving ? "处理中..." : "批准"}
                </button>
                <button
                  onClick={() => handleApproval("rejected")}
                  disabled={approving}
                  className="bg-red-600 hover:bg-red-700 text-white text-xs font-bold px-4 py-2 rounded-lg transition disabled:opacity-50"
                >
                  驳回
                </button>
              </div>
            </div>
            <div className="mt-2">
              <input
                type="text"
                placeholder="御批备注（可选）..."
                value={comment}
                onChange={(e) => setComment(e.target.value)}
                className="w-full px-3 py-1.5 border border-amber-300 rounded text-sm bg-white"
              />
            </div>
            {approvalError && <p className="text-xs text-red-600 mt-1">{approvalError}</p>}
          </div>
        )}

        {isShujiMarkdown && parsed.meta && <FrontmatterCard meta={parsed.meta} />}

        {isMarkdown ? (
          <article className="prose prose-sm max-w-none prose-headings:text-ink-900 prose-a:text-vermillion prose-code:text-vermillion prose-pre:bg-ink-900 prose-pre:text-ink-100">
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
    <div className="rounded-xl border border-ink-300 bg-[#1e1e1e] shadow-sm overflow-hidden">
      <div className="h-9 bg-[#252526] border-b border-[#3c3c3c] flex items-center justify-between text-[11px]">
        <div className="h-full px-3 bg-[#1e1e1e] border-r border-[#3c3c3c] flex items-center gap-2 text-[#d4d4d4] font-mono">
          <span className="text-[#858585]">{fileGlyph(path)}</span>
          <span className="truncate max-w-[520px]">{path.split("/").pop()}</span>
        </div>
        <div className="px-3 text-[#858585] font-mono flex items-center gap-3">
          <span>{language}</span>
          <span>{lines.length.toLocaleString()} lines</span>
          <span>{content.length.toLocaleString()} chars</span>
        </div>
      </div>
      <div className="overflow-auto max-h-[calc(100vh-190px)] text-[13px] leading-[20px] font-[Cascadia_Code,JetBrains_Mono,Consolas,Menlo,Monaco,monospace]">
        <table className="w-full border-separate border-spacing-0">
          <tbody>
            {lines.map((line, index) => (
              <tr key={index} className="hover:bg-white/[0.04]">
                <td className="select-none sticky left-0 bg-[#1e1e1e] w-14 min-w-14 pr-3 text-right text-[#858585] border-r border-[#2d2d2d] align-top">
                  {index + 1}
                </td>
                <td className="pl-4 pr-6 text-[#d4d4d4] whitespace-pre align-top">
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
    <div className="mb-5 rounded-xl border border-ink-200 bg-white p-4 shadow-sm">
      <div className="text-[10px] uppercase tracking-wider text-ink-400 mb-2">Frontmatter</div>
      <div className="grid grid-cols-1 sm:grid-cols-2 gap-2">
        {Object.entries(meta).map(([key, value]) => {
          const statusColor = key === "status" && value === "in_review" ? "text-amber-700 font-bold" :
            key === "status" && value === "approved" ? "text-green-700 font-bold" :
            key === "status" && value === "rejected" ? "text-red-700 font-bold" :
            "text-ink-700";
          if (key === "notes" && !value) return null;
          if (key === "status" && !value) return null;
          return (
            <div key={key} className="flex text-xs font-mono">
              <span className="w-20 shrink-0 text-ink-400">{labels[key] || key}</span>
              <span className={`break-all ${statusColor}`}>{value}</span>
            </div>
          );
        })}
      </div>
    </div>
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
