import { useEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { getDeptLogs } from "../api";
import type { DeptLogEntry } from "../types";

const MAX_ENTRIES = 200;

const DEPT_COLORS: Record<string, string> = {
  内阁: "text-purple-600",
  中书省: "text-blue-600",
  门下省: "text-cyan-600",
  尚书省: "text-orange-600",
  吏部: "text-green-600",
  兵部: "text-red-600",
  工部: "text-yellow-600",
  刑部: "text-gray-600",
  礼部: "text-indigo-600",
};

export default function DeptStatusPanel() {
  const [entries, setEntries] = useState<DeptLogEntry[]>([]);
  const scrollRef = useRef<HTMLDivElement>(null);

  // Load persisted history on mount (survives page navigation)
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

  if (entries.length === 0) return null;

  return (
    <div className="border rounded bg-gray-50 text-[11px] font-mono overflow-hidden flex flex-col">
      <div className="text-[10px] text-gray-400 px-2 py-1 border-b bg-white/50 shrink-0">
        部门日志
      </div>
      <div className="overflow-y-auto p-1.5 space-y-0.5 max-h-48">
        {entries.map((e, i) => (
          <div key={i} className="leading-4 whitespace-nowrap">
            <span className="text-gray-400">[{e.ts}]</span>{" "}
            <span className={DEPT_COLORS[e.dept] || "text-gray-600"}>{e.dept}</span>
            <span className="text-gray-500"> {e.action}</span>
          </div>
        ))}
        <div ref={scrollRef} />
      </div>
    </div>
  );
}
