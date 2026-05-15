import { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { getDeptLogs } from "../api";
import type { DeptLogEntry } from "../types";

const MAX_ENTRIES = 300;

const DEPT_COLORS: Record<string, string> = {
  内阁: "text-purple-600", 中书令: "text-blue-600",
  门下侍中: "text-cyan-600", 尚书令: "text-orange-600",
  吏部: "text-green-600", 兵部: "text-red-600",
  工部: "text-yellow-600", 刑部: "text-gray-600",
  礼部: "text-indigo-600",
};

const DEPT_DOT: Record<string, string> = {
  内阁: "bg-purple-500", 中书令: "bg-blue-500",
  门下侍中: "bg-cyan-500", 尚书令: "bg-orange-500",
  吏部: "bg-green-500", 兵部: "bg-red-500",
  工部: "bg-yellow-500", 刑部: "bg-gray-500",
  礼部: "bg-indigo-500",
};

function isRouteEntry(a: string) { return a.startsWith("→ "); }
function isErrorEntry(a: string) { return a.startsWith("❌"); }

function DeptDot({ dept }: { dept: string }) {
  return <span className={`w-1.5 h-1.5 rounded-full shrink-0 ${DEPT_DOT[dept] || "bg-gray-400"}`} />;
}

export default function DeptStatusPanel() {
  const [entries, setEntries] = useState<DeptLogEntry[]>([]);
  const [expanded, setExpanded] = useState<Set<number>>(new Set());

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

  const routes = entries.filter((e) => isRouteEntry(e.action));
  const errors = entries.filter((e) => isErrorEntry(e.action));
  const executions = entries.filter((e) => !isRouteEntry(e.action) && !isErrorEntry(e.action));

  return (
    <div className="h-full flex flex-col overflow-hidden bg-ink-100">
      {/* ── Panel 1: Route Events (auto-height, max 5 rows) ── */}
      <div className="flex flex-col border-b border-ink-200 shrink-0 max-h-[30%] min-h-[48px]">
        <div className="text-[10px] text-ink-400 px-2.5 py-1 bg-ink-200/30 shrink-0 font-medium tracking-wide">
          路由 {routes.length > 0 && `(${routes.length})`}
        </div>
        <div className="flex-1 overflow-y-auto min-h-0">
          {routes.map((e, i) => (
            <div key={i} className="flex items-center gap-1.5 px-2.5 py-0.5 text-[11px] font-mono hover:bg-ink-200/30 transition-colors">
              <DeptDot dept={e.dept} />
              <span className={`font-medium shrink-0 ${DEPT_COLORS[e.dept] || "text-gray-500"}`}>{e.dept}</span>
              <span className="text-vermillion">→</span>
              <span className="text-ink-700 truncate">{e.action.replace("→ ", "")}</span>
              <span className="text-ink-400 text-[10px] ml-auto shrink-0">{e.ts}</span>
            </div>
          ))}
          {routes.length === 0 && <div className="text-[10px] text-ink-400 px-2.5 py-1">暂无</div>}
        </div>
      </div>

      {/* ── Panel 2: Error Events (auto-height, max 3 cards) ── */}
      <div className="flex flex-col border-b border-ink-200 shrink-0 max-h-[25%] min-h-[48px]">
        <div className="text-[10px] text-vermillion-dark/60 px-2.5 py-1 bg-vermillion-light/50 shrink-0 font-medium tracking-wide">
          错误 {errors.length > 0 && `(${errors.length})`}
        </div>
        <div className="flex-1 overflow-y-auto min-h-0">
          {errors.map((e, i) => (
            <div key={i} className="mx-2 my-1 border border-vermillion/25 rounded bg-vermillion-light/60 px-2 py-1 text-[11px] font-mono">
              <div className="flex items-center gap-1.5">
                <DeptDot dept={e.dept} />
                <span className={`font-medium shrink-0 ${DEPT_COLORS[e.dept] || "text-gray-500"}`}>{e.dept}</span>
                <span className="text-vermillion-dark">{e.action}</span>
                <span className="text-ink-400 text-[10px] ml-auto shrink-0">{e.ts}</span>
              </div>
              {e.detail && (
                <div className="mt-0.5 ml-5 text-[10px] text-vermillion-dark/70 whitespace-pre-wrap break-all">
                  {e.detail}
                </div>
              )}
            </div>
          ))}
          {errors.length === 0 && <div className="text-[10px] text-ink-400 px-2.5 py-1">暂无</div>}
        </div>
      </div>

      {/* ── Panel 3: Execution Events (fills remaining space) ── */}
      <div className="flex flex-col flex-1 min-h-[80px]">
        <div className="text-[10px] text-ink-400 px-2.5 py-1 bg-ink-200/30 shrink-0 font-medium tracking-wide">
          执行 {executions.length > 0 && `(${executions.length})`}
        </div>
        <div className="flex-1 overflow-y-auto min-h-0">
          {executions.map((e, i) => {
            const hasDetail = !!e.detail;
            const idx = entries.indexOf(e);
            const open = expanded.has(idx);

            return (
              <div key={i}>
                <button
                  onClick={() => {
                    if (!hasDetail) return;
                    setExpanded((prev) => {
                      const next = new Set(prev);
                      open ? next.delete(idx) : next.add(idx);
                      return next;
                    });
                  }}
                  className={`w-full flex items-center gap-1.5 px-2.5 py-0.5 text-[11px] font-mono text-left transition-colors ${
                    hasDetail ? "cursor-pointer hover:bg-ink-200/30" : "cursor-default"
                  }`}
                >
                  <DeptDot dept={e.dept} />
                  <span className={`font-medium shrink-0 ${DEPT_COLORS[e.dept] || "text-gray-500"}`}>{e.dept}</span>
                  <span className="text-ink-600 truncate">{e.action}</span>
                  {hasDetail && (
                    <span className="text-ink-400 text-[10px] ml-auto shrink-0">{open ? "▾" : "▸"}</span>
                  )}
                  <span className="text-ink-400 text-[10px] ml-1 shrink-0">{e.ts}</span>
                </button>
                {open && hasDetail && (
                  <div className="ml-7 mr-2 mb-0.5 text-[10px] text-ink-500 whitespace-pre-wrap break-all border-l-2 border-ink-300 pl-2 pr-1 py-0.5 bg-ink-200/20 rounded-r">
                    {e.detail}
                  </div>
                )}
              </div>
            );
          })}
          {executions.length === 0 && <div className="text-[10px] text-ink-400 px-2.5 py-1">暂无</div>}
        </div>
      </div>
    </div>
  );
}
