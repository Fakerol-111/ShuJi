import { useEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { getDeptLogs } from "../api";
import type { DeptLogEntry } from "../types";

const MAX_ENTRIES = 300;

const DEPT_COLORS: Record<string, string> = {
  内阁: "text-purple-600",
  中书令: "text-blue-600",
  门下侍中: "text-cyan-600",
  尚书令: "text-orange-600",
  吏部: "text-green-600",
  兵部: "text-red-600",
  工部: "text-yellow-600",
  刑部: "text-gray-600",
  礼部: "text-indigo-600",
};

function DeptBadge({ dept }: { dept: string }) {
  return (
    <span className={`font-medium shrink-0 ${DEPT_COLORS[dept] || "text-gray-500"}`}>
      {dept}
    </span>
  );
}

function isRouteEntry(action: string): boolean {
  return action.startsWith("→ ") && !action.startsWith("→ →");
}

function isErrorEntry(action: string): boolean {
  return action.startsWith("❌");
}

export default function DeptStatusPanel() {
  const [entries, setEntries] = useState<DeptLogEntry[]>([]);
  const scrollRef = useRef<HTMLDivElement>(null);

  // Load persisted history on mount
  useEffect(() => {
    getDeptLogs().then((hist) => {
      if (hist.length > 0) {
        setEntries(hist.slice(-MAX_ENTRIES));
      }
    }).catch(() => {});
  }, []);

  // Listen for real-time events
  useEffect(() => {
    const unlisten = listen<DeptLogEntry>("dept-log", (event) => {
      setEntries((prev) => {
        const next = [...prev, event.payload];
        return next.length > MAX_ENTRIES ? next.slice(-MAX_ENTRIES) : next;
      });
    });
    return () => { unlisten.then((f) => f()); };
  }, []);

  useEffect(() => {
    scrollRef.current?.scrollIntoView({ behavior: "smooth" });
  }, [entries]);

  return (
    <div className="h-full flex flex-col overflow-hidden bg-gray-50">
      <div className="text-[10px] text-gray-400 px-3 py-2 border-b bg-white/50 shrink-0 font-medium">
        系统日志
      </div>
      <div className="flex-1 overflow-y-auto p-2 space-y-0.5 font-mono text-[11px]">
        {entries.length === 0 && (
          <div className="text-gray-400 text-center py-8">等待系统活动...</div>
        )}
        {entries.map((e, i) => {
          const action = e.action;
          const route = isRouteEntry(action);
          const err = isErrorEntry(action);

          return (
            <div
              key={i}
              className={`leading-relaxed rounded px-1.5 py-0.5 ${
                err ? "bg-red-50 text-red-700" : route ? "text-gray-600" : "text-gray-700"
              }`}
            >
              <span className="text-gray-300 text-[10px] mr-1.5">{e.ts}</span>
              <DeptBadge dept={e.dept} />
              {route ? (
                <span className="text-blue-600 ml-1">&#8627;</span>
              ) : (
                <span className="text-gray-400 ml-1">&#8226;</span>
              )}
              <span className={err ? "text-red-600" : "text-gray-600"}> {action}</span>
              {e.detail && (
                <div className="mt-0.5 ml-8 text-[10px] text-gray-400 whitespace-pre-wrap break-all border-l-2 border-gray-200 pl-2">
                  {e.detail}
                </div>
              )}
            </div>
          );
        })}
        <div ref={scrollRef} />
      </div>
    </div>
  );
}
