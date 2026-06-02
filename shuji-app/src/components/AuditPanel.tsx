import { useEffect, useState } from "react";
import { getAuditTimeline } from "../api";
import type { TimelineData } from "../types";

const EVENT_LABELS: Record<string, string> = {
  create_document: "创建文档",
  set_document_status: "状态变更",
  checkpoint: "存档",
  milestone: "里程碑",
};

const EVENT_COLORS: Record<string, string> = {
  create_document: "text-jade",
  set_document_status: "text-gold",
  checkpoint: "text-info",
  milestone: "text-ink-500",
};

export default function AuditPanel() {
  const [data, setData] = useState<TimelineData | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState("");

  useEffect(() => {
    setLoading(true);
    getAuditTimeline()
      .then(setData)
      .catch((e) => setError(String(e)))
      .finally(() => setLoading(false));
  }, []);

  if (loading) {
    return <div className="p-4 text-body text-ink-400 text-center mt-8">载入中…</div>;
  }

  if (error) {
    return (
      <div className="p-4">
        <div className="rounded-lg bg-vermillion/10 border border-vermillion/20 px-3 py-2 text-caption text-vermillion">
          {error}
        </div>
      </div>
    );
  }

  if (!data || data.entries.length === 0) {
    return <div className="p-4 text-body text-ink-400 text-center mt-8">尚无朝报记录</div>;
  }

  return (
    <div className="h-full flex flex-col">
      {/* Summary header */}
      <div className="px-3 py-2 border-b border-fold space-y-1">
        <div className="text-caption text-ink-500">
          共 {data.summary.total_events} 条记录
        </div>
        <div className="flex flex-wrap gap-1.5">
          {data.summary.by_event.slice(0, 5).map(([evt, count]) => (
            <span
              key={evt}
              className="px-1.5 py-0.5 rounded bg-ink-100 text-caption text-ink-600"
            >
              {EVENT_LABELS[evt] || evt} {count}
            </span>
          ))}
        </div>
      </div>

      {/* Entry list */}
      <div className="flex-1 overflow-y-auto min-h-0">
        {[...data.entries].reverse().map((entry, i) => {
          const color = EVENT_COLORS[entry.event] || "text-ink-500";
          return (
            <div
              key={i}
              className="px-3 py-1.5 border-b border-fold/50 hover:bg-ink-100/30 transition-colors"
            >
              <div className="flex items-center gap-1.5 text-caption">
                <span className={`font-mono ${color}`}>
                  {EVENT_LABELS[entry.event] || entry.event}
                </span>
                <span className="text-ink-400">{entry.role}</span>
                {entry.doc_id && (
                  <span className="text-ink-500 font-mono">{entry.doc_id}</span>
                )}
              </div>
              <div className="text-caption text-ink-400 mt-0.5 truncate">
                {entry.detail || "—"}
              </div>
              <div className="text-[9px] text-ink-300 font-mono mt-0.5">{entry.ts}</div>
            </div>
          );
        })}
      </div>
    </div>
  );
}
