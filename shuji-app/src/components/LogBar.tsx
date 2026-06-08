import { useEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { getDeptLogs } from "../api";
import type { DeptLogEntry } from "../types";
import DeptStatusPanel from "./DeptStatusPanel";
import { getDeptMeta } from "../constants";

interface LogBarProps {
  expanded: boolean;
  onExpandedChange: (expanded: boolean) => void;
}

export default function LogBar({ expanded, onExpandedChange }: LogBarProps) {
  const [latest, setLatest] = useState<DeptLogEntry | null>(null);
  const containerRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    getDeptLogs()
      .then((logs) => setLatest(logs.length > 0 ? logs[logs.length - 1] : null))
      .catch((e) => console.error("日志加载失败:", e));
  }, []);

  useEffect(() => {
    const unlisten = listen<DeptLogEntry>("dept-log", (event) =>
      setLatest(event.payload),
    );
    return () => {
      unlisten.then((f) => f());
    };
  }, []);

  useEffect(() => {
    if (expanded && containerRef.current) {
      containerRef.current.scrollTop = containerRef.current.scrollHeight;
    }
  }, [expanded, latest]);

  return (
    <div
      className={`${expanded ? "h-52" : "h-6"} bg-ink-100 border-t border-ink-300 shrink-0 flex flex-col transition-[height] duration-150`}
    >
      <button
        onClick={() => onExpandedChange(!expanded)}
        className="h-6 px-3 flex items-center gap-2 text-[10px] font-mono text-left hover:bg-ink-200/70 shrink-0"
      >
        <span className="text-ink-500">{expanded ? "▾" : "▸"} 日志</span>
        {latest ? (
          <>
            <span className="text-ink-400">{latest.ts}</span>
            <span
              style={{ color: getDeptMeta(latest.dept)?.color || "#6b7280" }}
            >
              {latest.dept}
            </span>
            <span className="text-ink-400">→</span>
            <span className="text-ink-600 truncate">{latest.action}</span>
          </>
        ) : (
          <span className="text-ink-400">暂无日志</span>
        )}
      </button>
      {expanded && (
        <div
          ref={containerRef}
          className="flex-1 min-h-0 overflow-hidden border-t border-ink-200"
        >
          <DeptStatusPanel />
        </div>
      )}
    </div>
  );
}
