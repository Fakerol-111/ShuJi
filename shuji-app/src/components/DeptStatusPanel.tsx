import { useEffect, useState, useRef } from 'react';
import { listen } from '@tauri-apps/api/event';
import { getDeptLogs } from '../api';
import { getDeptMeta } from '../constants';
import type { DeptLogEntry } from '../types';

const MAX_ENTRIES = 300;

function isRouteEntry(a: string) {
  return a.startsWith('→ ');
}
function isErrorEntry(a: string) {
  return a.startsWith('❌');
}

export default function DeptStatusPanel() {
  const [entries, setEntries] = useState<DeptLogEntry[]>([]);
  const [expanded, setExpanded] = useState<Set<number>>(new Set());
  const containerRef = useRef<HTMLDivElement>(null);
  const [autoScroll, setAutoScroll] = useState(true);

  useEffect(() => {
    getDeptLogs()
      .then((hist) => {
        if (hist.length > 0) setEntries(hist.slice(-MAX_ENTRIES));
      })
      .catch((e) => console.error('部门日志加载失败:', e));
  }, []);

  useEffect(() => {
    const unlisten = listen<DeptLogEntry>('dept-log', (event) => {
      setEntries((prev) => {
        const next = [...prev, event.payload];
        return next.length > MAX_ENTRIES ? next.slice(-MAX_ENTRIES) : next;
      });
    });
    return () => {
      unlisten.then((f) => f());
    };
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
          {entries.filter((e) => isRouteEntry(e.action)).length} 路由 ·{' '}
          {entries.filter((e) => isErrorEntry(e.action)).length} 错误
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
            const routeAccent =
              getDeptMeta(e.dept)?.accent?.replace('border-l-', 'bg-') || 'bg-gray-300';
            return (
              <div
                key={i}
                className="flex items-center gap-1.5 text-[10px] font-mono py-0.5 px-1 opacity-60 hover:opacity-100 transition-opacity"
              >
                <span
                  className={`w-1 h-1 rounded-full shrink-0 ${e.dept ? routeAccent : 'bg-gray-300'}`}
                />
                <span className="font-medium text-ink-500 shrink-0">{e.dept}</span>
                <span className="text-vermillion/70 shrink-0">→</span>
                <span className="text-ink-500 truncate">{e.action.replace('→ ', '')}</span>
                <span className="text-ink-400 ml-auto shrink-0">{e.ts}</span>
              </div>
            );
          }

          if (error) {
            return (
              <div
                key={i}
                className="rounded-lg border border-red-200 bg-red-50/80 px-2 py-1 text-[10px] font-mono"
              >
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
          const meta = getDeptMeta(e.dept);
          const accent = meta?.accent || 'border-l-gray-300';
          const bg = meta?.bg ? `${meta.bg} border-current/10` : 'bg-gray-50 border-gray-200';
          const txt = meta?.text || 'text-gray-600';

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
                  hasDetail ? 'cursor-pointer hover:brightness-95' : 'cursor-default'
                }`}
              >
                <div className="flex items-center gap-1.5">
                  <span className={`font-medium ${txt} shrink-0`}>{e.dept}</span>
                  <span className="text-ink-600 truncate flex-1">{e.action}</span>
                  {hasDetail && <span className="text-ink-400 shrink-0">{open ? '▾' : '▸'}</span>}
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
