import { useEffect, useState } from "react";
import { getContextStats } from "../api";
import type { ContextStats } from "../api";

const ROLE_NAMES: Record<string, string> = {
  zhongshuling: "中书令", menxiashizhong: "门下侍中", neige: "内阁",
  shangshuling: "尚书令", libushangshu: "吏部", bingbushangshu: "兵部",
  gongbushangshu: "工部", xingbushangshu: "刑部", liburshangshu: "礼部",
  zhisi: "制司",
};

export default function ContextPanel() {
  const [stats, setStats] = useState<Record<string, ContextStats> | null>(null);
  const [error, setError] = useState("");

  const load = () => {
    setError("");
    getContextStats().then(setStats).catch((e) => setError(String(e)));
  };

  useEffect(load, []);
  useEffect(() => {
    const id = setInterval(load, 10000);
    return () => clearInterval(id);
  }, []);

  return (
    <div className="h-full overflow-y-auto p-3 bg-ink-50">
      <div className="flex items-center justify-between mb-3">
        <h3 className="text-xs font-bold text-ink-800">文脉</h3>
        <button onClick={load} className="text-[10px] text-ink-400 hover:text-ink-700">刷新</button>
      </div>
      {error && <p className="text-xs text-vermillion mb-2">{error}</p>}
      {!stats || Object.keys(stats).length === 0 ? (
        <p className="text-xs text-ink-400">暂无数据</p>
      ) : (
        <div className="space-y-3">
          {Object.entries(stats).sort(([a], [b]) => roleOrder(a) - roleOrder(b)).map(([role, cs]) => {
            const pct = cs.char_threshold > 0
              ? Math.min((cs.char_count / cs.char_threshold) * 100, 100)
              : 0;
            const historyPct = cs.history_threshold > 0
              ? Math.min((cs.history_char_count / cs.history_threshold) * 100, 100)
              : 0;
            return (
              <div key={role}>
                <div className="flex justify-between text-[11px] mb-1">
                  <span className="font-medium text-ink-700">{ROLE_NAMES[role] || role}</span>
                  <span className={cs.char_count >= cs.char_threshold ? "text-vermillion text-[10px]" : "text-ink-500"}>
                    {abbr(cs.char_count)} / {abbr(cs.char_threshold)}
                  </span>
                </div>
                <div className="w-full bg-ink-200 rounded-full h-2 overflow-hidden">
                  <div
                    className="h-full rounded-full transition-all duration-500"
                    style={{ width: `${Math.max(pct, 2)}%`, background: barColor(role) }}
                  />
                </div>
                <div className="flex justify-between items-center text-[9px] text-ink-400 mt-0.5">
                  <span>{cs.message_count} 条消息</span>
                  {cs.compressed ? (
                    <span className="text-amber-600 flex items-center gap-1">
                      <span>已压缩</span>
                      <span className="text-ink-400">(历史 {abbr(cs.history_char_count)} / {abbr(cs.history_threshold)})</span>
                    </span>
                  ) : (
                    <span className="text-ink-400">未压缩</span>
                  )}
                </div>
                {cs.compressed && (
                  <div className="w-full bg-ink-200 rounded-full h-1 mt-0.5 overflow-hidden">
                    <div className="h-full rounded-full bg-amber-400 transition-all duration-500" style={{ width: `${Math.max(historyPct, 2)}%` }} />
                  </div>
                )}
              </div>
            );
          })}
        </div>
      )}
    </div>
  );
}

function abbr(n: number): string {
  if (n >= 1000000) return (n / 10000).toFixed(0) + "w";
  if (n >= 10000) return (n / 10000).toFixed(1) + "w";
  if (n >= 1000) return (n / 1000).toFixed(1) + "k";
  return String(n);
}

function roleOrder(role: string) {
  const order = ["neige", "zhongshuling", "menxiashizhong", "shangshuling", "libushangshu", "bingbushangshu", "gongbushangshu", "xingbushangshu", "liburshangshu", "zhisi",
    "内阁", "中书令", "门下侍中", "尚书令", "吏部", "兵部", "工部", "刑部", "礼部", "制司"];
  const idx = order.indexOf(role);
  return idx < 0 ? 999 : idx;
}

const palette = ["#3b82f6", "#10b981", "#f59e0b", "#ef4444", "#8b5cf6", "#06b6d4", "#ec4899", "#84cc16", "#14b8a6", "#f97316"];
function barColor(role: string) {
  return palette[Math.abs(hash(role)) % palette.length];
}
function hash(s: string) {
  return s.split("").reduce((n, ch) => n + ch.charCodeAt(0), 0);
}
