import { useEffect, useState } from "react";
import { getContextStats, compactContext } from "../api";
import type { ContextStats } from "../api";

const ROLE_NAMES: Record<string, string> = {
  zhongshuling: "中书令", menxiashizhong: "门下侍中", neige: "内阁",
  shangshuling: "尚书令", libushangshu: "吏部", bingbushangshu: "兵部",
  gongbushangshu: "工部", xingbushangshu: "刑部", liburshangshu: "礼部",
};

export default function ContextPanel() {
  const [stats, setStats] = useState<Record<string, ContextStats> | null>(null);
  const [error, setError] = useState("");
  const [compactingRole, setCompactingRole] = useState<string | null>(null);
  const [lastCompactMsg, setLastCompactMsg] = useState("");

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
      <div className="flex items-center justify-between mb-1">
        <h3 className="text-xs font-bold text-ink-800">文脉</h3>
        <button onClick={load} className="text-[10px] text-ink-400 hover:text-ink-700">刷新</button>
      </div>
      <p className="text-[9px] text-ink-400 mb-3">单位：tokens（cl100k，与 OpenAI/DeepSeek 兼容 API 估算一致）</p>
      {error && <p className="text-xs text-vermillion mb-2">{error}</p>}
      {!stats || Object.keys(stats).length === 0 ? (
        <p className="text-xs text-ink-400">暂无数据</p>
      ) : (
        <div className="space-y-3">
          {Object.entries(stats).sort(([a], [b]) => roleOrder(a) - roleOrder(b)).map(([role, cs]) => {
            const pct = cs.token_threshold > 0
              ? Math.min((cs.token_count / cs.token_threshold) * 100, 100)
              : 0;
            return (
              <div key={role}>
                <div className="flex justify-between text-[11px] mb-1">
                  <span className="font-medium text-ink-700">{ROLE_NAMES[role] || role}</span>
                  <span className={cs.token_count >= cs.token_threshold ? "text-vermillion text-[10px]" : "text-ink-500"}>
                    {abbr(cs.token_count)} / {abbr(cs.token_threshold)} tokens
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
                    <span className="text-amber-600">已压缩</span>
                  ) : (
                    <span className="text-ink-400">未压缩</span>
                  )}
                </div>
                <div className="mt-1">
                    <button
                      onClick={async () => {
                        setCompactingRole(role);
                        setLastCompactMsg("");
                        setError("");
                        try {
                          const msg = await compactContext(role);
                          setLastCompactMsg(msg);
                          const newStats = await getContextStats();
                          setStats(newStats);
                        } catch (e) {
                          setError(String(e));
                        } finally {
                          setCompactingRole(null);
                        }
                      }}
                      disabled={compactingRole !== null}
                      className={`text-[10px] px-2 py-0.5 rounded border transition-colors disabled:opacity-50 disabled:cursor-not-allowed ${
                        compactingRole === role
                          ? "bg-amber-100 border-amber-300 text-amber-700"
                          : "border-ink-300 hover:bg-ink-100 text-ink-600"
                      }`}
                    >
                      {compactingRole === role ? "压缩中…" : "立即压缩"}
                    </button>
                    {lastCompactMsg && compactingRole === null && (
                      <span className="text-[9px] text-jade ml-1.5">{lastCompactMsg}</span>
                    )}
                  </div>
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
  const order = ["neige", "zhongshuling", "menxiashizhong", "shangshuling", "libushangshu", "bingbushangshu", "gongbushangshu", "xingbushangshu", "liburshangshu",
    "内阁", "中书令", "门下侍中", "尚书令", "吏部", "兵部", "工部", "刑部", "礼部"];
  const idx = order.indexOf(role);
  return idx < 0 ? 999 : idx;
}

const palette = ["#3b82f6", "#10b981", "#f59e0b", "#ef4444", "#8b5cf6", "#06b6d4", "#ec4899", "#84cc16", "#14b8a6"];
function barColor(role: string) {
  return palette[Math.abs(hash(role)) % palette.length];
}
function hash(s: string) {
  return s.split("").reduce((n, ch) => n + ch.charCodeAt(0), 0);
}
