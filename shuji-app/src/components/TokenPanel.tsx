import { useEffect, useState } from "react";
import { getTokenStats } from "../api";
import type { TokenUsage } from "../api";

const ROLE_NAMES: Record<string, string> = {
  zhongshu: "中书省", menxia: "门下省", neige: "内阁",
  shangshu: "尚书省", libup: "吏部", bingbu: "兵部",
  gongbu: "工部", xingbu: "刑部", libur: "礼部",
  hubu: "户部", zhisi: "制司",
};

export default function TokenPanel() {
  const [stats, setStats] = useState<Record<string, Record<string, TokenUsage>> | null>(null);
  const [windowName, setWindowName] = useState("汇总");
  const [error, setError] = useState("");

  const load = () => {
    setError("");
    getTokenStats().then(setStats).catch((e) => setError(String(e)));
  };

  useEffect(load, []);

  const current = stats?.[windowName] || {};
  const maxTotal = Math.max(...Object.values(current).map((u) => u.total_tokens), 1);

  return (
    <div className="h-full overflow-y-auto p-3 bg-ink-50">
      <div className="flex items-center justify-between mb-3">
        <h3 className="text-xs font-bold text-ink-800">度支</h3>
        <button onClick={load} className="text-[10px] text-ink-400 hover:text-ink-700">刷新</button>
      </div>
      {stats && Object.keys(stats).length > 0 && (
        <div className="flex gap-1 mb-3 flex-wrap">
          {["今日", "近3日", "近7日", "汇总"].filter((w) => stats[w]).map((w) => (
            <button
              key={w}
              onClick={() => setWindowName(w)}
              className={`text-[10px] px-2 py-1 rounded ${windowName === w ? "bg-ink-900 text-white" : "bg-ink-100 text-ink-500 hover:bg-ink-200"}`}
            >
              {w}
            </button>
          ))}
        </div>
      )}
      {error && <p className="text-xs text-vermillion mb-2">{error}</p>}
      {!stats || Object.keys(stats).length === 0 ? (
        <p className="text-xs text-ink-400">暂无数据</p>
      ) : (
        <div className="space-y-4">
          {Object.entries(current).sort(([a], [b]) => roleOrder(a) - roleOrder(b)).map(([role, usage]) => {
            const pct = (usage.total_tokens / maxTotal) * 100;
            return (
              <div key={role}>
                <div className="flex justify-between text-[11px] mb-1">
                  <span className="font-medium text-ink-700">{ROLE_NAMES[role] || role}</span>
                  <span className="text-ink-500">{usage.total_tokens.toLocaleString()}</span>
                </div>
                <div className="w-full bg-ink-200 rounded-full h-2 overflow-hidden">
                  <div className="h-full rounded-full transition-all duration-500" style={{ width: `${Math.max(pct, 2)}%`, background: barColor(role) }} />
                </div>
                <div className="flex justify-between text-[9px] text-ink-400 mt-0.5">
                  <span>调用 {usage.call_count} 次</span>
                  <span>入 {usage.prompt_tokens.toLocaleString()} / 出 {usage.completion_tokens.toLocaleString()}</span>
                </div>
              </div>
            );
          })}
        </div>
      )}
    </div>
  );
}

function roleOrder(role: string) {
  const order = ["内阁", "中书令", "门下侍中", "尚书令", "吏部", "兵部", "工部", "礼部", "刑部", "neige", "zhongshu", "menxia", "shangshu", "libup", "bingbu", "gongbu", "libur", "xingbu"];
  const idx = order.indexOf(role);
  return idx < 0 ? 999 : idx;
}

const palette = ["#3b82f6", "#10b981", "#f59e0b", "#ef4444", "#8b5cf6", "#06b6d4", "#ec4899", "#84cc16", "#14b8a6", "#f97316", "#6366f1"];
function barColor(role: string) {
  return palette[Math.abs(hash(role)) % palette.length];
}
function hash(s: string) {
  return s.split("").reduce((n, ch) => n + ch.charCodeAt(0), 0);
}
