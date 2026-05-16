import { useEffect, useState, useRef } from "react";
import { listen } from "@tauri-apps/api/event";
import { getDeptLogs } from "../api";
import type { DeptLogEntry } from "../types";

const MAX_ENTRIES = 300;

const DEPT_BG: Record<string, string> = {
  内阁: "bg-purple-50 border-purple-200", 中书令: "bg-blue-50 border-blue-200",
  门下侍中: "bg-cyan-50 border-cyan-200", 尚书令: "bg-orange-50 border-orange-200",
  吏部: "bg-green-50 border-green-200", 兵部: "bg-red-50 border-red-200",
  工部: "bg-amber-50 border-amber-200", 刑部: "bg-gray-50 border-gray-200",
  礼部: "bg-indigo-50 border-indigo-200",
};

const DEPT_ACCENT: Record<string, string> = {
  内阁: "border-l-purple-400", 中书令: "border-l-blue-400",
  门下侍中: "border-l-cyan-400", 尚书令: "border-l-orange-400",
  吏部: "border-l-green-400", 兵部: "border-l-red-400",
  工部: "border-l-amber-400", 刑部: "border-l-gray-400",
  礼部: "border-l-indigo-400",
};

const DEPT_TEXT: Record<string, string> = {
  内阁: "text-purple-700", 中书令: "text-blue-700",
  门下侍中: "text-cyan-700", 尚书令: "text-orange-700",
  吏部: "text-green-700", 兵部: "text-red-700",
  工部: "text-amber-700", 刑部: "text-gray-600",
  礼部: "text-indigo-700",
};

function isRouteEntry(a: string) { return a.startsWith("→ "); }
function isErrorEntry(a: string) { return a.startsWith("❌"); }

export default function DeptStatusPanel() {
  const [entries, setEntries] = useState<DeptLogEntry[]>([]);
  const [expanded, setExpanded] = useState<Set<number>>(new Set());
  const containerRef = useRef<HTMLDivElement>(null);
  const [autoScroll, setAutoScroll] = useState(true);

  useEffect(() => {
    getDeptLogs().then((hist) => {
      if (hist.length > 0) setEntries(hist.slice(-MAX_ENTRIES));
    }).catch(() => {});
  }, []);

  useEffect(() => {
    const unlisten = listen<DeptLogEntry>("dept-log", (event) => {
      setEntries((prev) => {
        const next = [...prev, event.payload];
        return next.length > MAX_ENTRIES ? next.slice(-MAX_ENTRIES) : next;
      });
    });
    return () => { unlisten.then((f) => f()); };
  }, []);

  // Auto-scroll to bottom when new entries arrive (if user hasn't scrolled up)
  const handleScroll = () => {
    const el = containerRef.current;
    if (!el) return;
    const atBottom = el.scrollHeight - el.scrollTop - el.clientHeight < 40;
    setAutoScroll(atBottom);
  };

  useEffect(() => {
    if (autoScroll && containerRef.current) {
      containerRef.current.scrollTop = containerRef.current.scrollHeight;
    }
  }, [entries, autoScroll]);

  return (
    <div className="h-full flex flex-col overflow-hidden bg-ink-100">
      {/* Header */}
      <div className="text-[10px] text-ink-400 px-3 py-1.5 bg-ink-200/40 shrink-0 font-medium tracking-wide flex items-center justify-between">
        <span>六部日志 · {entries.length}</span>
        <span className="text-ink-400/60">
          {entries.filter(e => isRouteEntry(e.action)).length} 路由 · {entries.filter(e => isErrorEntry(e.action)).length} 错误
        </span>
      </div>

      {/* Unified bubble feed */}
      <div
        ref={containerRef}
        onScroll={handleScroll}
        className="flex-1 overflow-y-auto min-h-0 px-2 py-1.5 space-y-1"
      >
        {entries.length === 0 && (
          <div className="text-[10px] text-ink-400 text-center py-8">暂无日志</div>
        )}
        {entries.map((e, i) => {
          const route = isRouteEntry(e.action);
          const error = isErrorEntry(e.action);
          const hasDetail = !!e.detail;
          const open = expanded.has(i);

          if (route) {
            return (
              <div key={i} className="flex items-center gap-1.5 text-[10px] font-mono py-0.5 px-1 opacity-60 hover:opacity-100 transition-opacity">
                <span className={`w-1 h-1 rounded-full shrink-0 ${e.dept ? DEPT_ACCENT[e.dept]?.replace("border-l-", "bg-") || "bg-gray-300" : "bg-gray-300"}`} />
                <span className="font-medium text-ink-500 shrink-0">{e.dept}</span>
                <span className="text-vermillion/70 shrink-0">→</span>
                <span className="text-ink-500 truncate">{e.action.replace("→ ", "")}</span>
                <span className="text-ink-400 ml-auto shrink-0">{e.ts}</span>
              </div>
            );
          }

          if (error) {
            return (
              <div key={i} className="rounded-lg border border-red-200 bg-red-50/80 px-2 py-1 text-[10px] font-mono">
                <div className="flex items-center gap-1.5">
                  <span className="w-1.5 h-1.5 rounded-full bg-red-400 shrink-0" />
                  <span className="font-medium text-red-600 shrink-0">{e.dept}</span>
                  <span className="text-red-700 truncate flex-1">{e.action}</span>
                  <span className="text-ink-400 shrink-0">{e.ts}</span>
                </div>
                {hasDetail && (
                  <div className="mt-0.5 ml-5 text-[9px] text-red-600/70 whitespace-pre-wrap break-all">
                    {e.detail}
                  </div>
                )}
              </div>
            );
          }

          // Execution bubble
          const accent = DEPT_ACCENT[e.dept] || "border-l-gray-300";
          const bg = DEPT_BG[e.dept] || "bg-gray-50 border-gray-200";
          const txt = DEPT_TEXT[e.dept] || "text-gray-600";

          return (
            <div key={i}>
              <button
                onClick={() => {
                  if (!hasDetail) return;
                  setExpanded((prev) => {
                    const next = new Set(prev);
                    open ? next.delete(i) : next.add(i);
                    return next;
                  });
                }}
                className={`w-full text-left rounded-lg border-l-2 ${accent} ${bg} px-2 py-1 text-[10px] transition-colors ${
                  hasDetail ? "cursor-pointer hover:brightness-95" : "cursor-default"
                }`}
              >
                <div className="flex items-center gap-1.5">
                  <span className={`font-medium ${txt} shrink-0`}>{e.dept}</span>
                  <span className="text-ink-600 truncate flex-1">{e.action}</span>
                  {hasDetail && (
                    <span className="text-ink-400 shrink-0">{open ? "▾" : "▸"}</span>
                  )}
                  <span className="text-ink-400 shrink-0">{e.ts}</span>
                </div>
              </button>
              {open && hasDetail && (
                <div className="ml-3 mr-1 mt-0.5 text-[9px] text-ink-500 whitespace-pre-wrap break-all border-l-2 border-ink-300 pl-2 pr-1 py-0.5 bg-ink-200/20 rounded-r font-mono">
                  {e.detail}
                </div>
              )}
            </div>
          );
        })}
      </div>
    </div>
  );
}
