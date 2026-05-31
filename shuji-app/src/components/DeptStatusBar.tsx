import { useEffect, useState } from "react";
import { getTokenStats, getRoundMetrics } from "../api";
import { useActiveDepts } from "../hooks/useActiveDepts";
import type { RoundMetrics } from "../types";

export const DEPT_COLORS: Record<string, string> = {
  内阁: "#7c3aed",
  中书令: "#3b82f6",
  门下侍中: "#06b6d4",
  尚书令: "#f97316",
  吏部: "#22c55e",
  兵部: "#ef4444",
  工部: "#f59e0b",
  刑部: "#6b7280",
  礼部: "#6366f1",
  户部: "#14b8a6",
  制司: "#ec4899",
};

const ROLE_ORDER = ["内阁", "中书令", "门下侍中", "尚书令", "吏部", "兵部", "工部", "刑部", "礼部", "户部", "制司"];

const SKILL_LABELS: Record<string, string> = {
  workflow_standard: "标准",
  workflow_demo: "演示",
  workflow_simple: "简单",
  workflow_complex: "复杂",
  workflow_optimize: "优化",
  workflow_bugfix: "修复",
  workflow_refactor: "重构",
  workflow_audit: "审计",
  discuss: "讨论",
  summary: "总结",
  clarify: "澄清",
};

export default function DeptStatusBar() {
  const active = useActiveDepts();
  const [tokenTotal, setTokenTotal] = useState(0);
  const [round, setRound] = useState<RoundMetrics | null>(null);
  const [elapsed, setElapsed] = useState("");

  useEffect(() => {
    const load = () => {
      getTokenStats().then((stats) => {
        const total = Object.values(stats["汇总"] || {}).reduce((sum, u) => sum + u.total_tokens, 0);
        setTokenTotal(total);
      }).catch((e) => console.error("Token统计加载失败:", e));
    };
    load();
    const timer = window.setInterval(load, 30000);
    return () => window.clearInterval(timer);
  }, []);

  // Poll round metrics every 3s
  useEffect(() => {
    const load = () => {
      getRoundMetrics().then((m) => {
        setRound(m);
      }).catch(() => {});
    };
    load();
    const timer = window.setInterval(load, 3000);
    return () => window.clearInterval(timer);
  }, []);

  // Update elapsed time every second when round is active
  useEffect(() => {
    if (!round) { setElapsed(""); return; }
    const tick = () => {
      const secs = Math.floor((Date.now() - round.started_at) / 1000);
      if (secs < 60) setElapsed(`${secs}s`);
      else if (secs < 3600) setElapsed(`${Math.floor(secs / 60)}min${secs % 60}s`);
      else setElapsed(`${Math.floor(secs / 3600)}h${Math.floor((secs % 3600) / 60)}min`);
    };
    tick();
    const timer = window.setInterval(tick, 1000);
    return () => window.clearInterval(timer);
  }, [round]);

  const visible = ROLE_ORDER.slice(0, 10);
  const hidden = ROLE_ORDER.slice(10);

  // Build round summary string
  const roundParts: string[] = [];
  if (round && round.started_at > 0) {
    if (round.current_role) roundParts.push(round.current_role);
    if (round.skill) roundParts.push(SKILL_LABELS[round.skill] || round.skill);
    // Show top active dept iteration
    const iterEntries = Object.entries(round.dept_iterations).filter(([, c]) => c > 0);
    if (iterEntries.length > 0) {
      const top = iterEntries.sort((a, b) => b[1] - a[1])[0];
      roundParts.push(`${top[0]}(${top[1]}次)`);
    }
    if (round.total_tokens > 0) roundParts.push(formatToken(round.total_tokens) + " tokens");
    if (elapsed) roundParts.push(elapsed);
    // Rough RMB estimate (DeepSeek approximate: ¥2/1M input, ¥8/1M output)
    const cost = (round.prompt_tokens * 2 + round.completion_tokens * 8) / 1_000_000;
    if (cost > 0.001) roundParts.push(`¥${cost.toFixed(4)}`);
  }

  return (
    <div className="h-6 bg-ink-900 text-ink-300 border-t border-ink-800 px-3 flex items-center justify-between text-[11px] shrink-0">
      <div className="flex items-center gap-2 min-w-0">
        <span className="text-amber-300 shrink-0">⚡</span>
        {visible.map((dept) => <DeptLight key={dept} dept={dept} active={active.has(dept)} />)}
        {hidden.length > 0 && (
          <span className="group relative text-ink-500 cursor-default">
            +{hidden.length} 更多
            <span className="hidden group-hover:flex absolute bottom-5 left-0 bg-ink-800 border border-ink-700 rounded px-2 py-1 gap-2 whitespace-nowrap shadow-lg">
              {hidden.map((dept) => <DeptLight key={dept} dept={dept} active={active.has(dept)} />)}
            </span>
          </span>
        )}
      </div>

      {/* Round metrics (center) */}
      {roundParts.length > 0 && (
        <div className="text-ink-400 font-mono truncate mx-2 text-[10px]" title={roundParts.join(" · ")}>
          {roundParts.join(" · ")}
        </div>
      )}

      <div className="font-mono text-ink-400 shrink-0 ml-3">token {formatToken(tokenTotal)}</div>
    </div>
  );
}

function DeptLight({ dept, active }: { dept: string; active: boolean }) {
  const color = DEPT_COLORS[dept] || "#9ca3af";
  return (
    <span className="flex items-center gap-0.5 whitespace-nowrap">
      <span>{dept}</span>
      <span className={active ? "animate-pulse" : ""} style={{ color }}>{active ? "●" : "○"}</span>
    </span>
  );
}

function formatToken(n: number) {
  if (n >= 1000) return `${(n / 1000).toFixed(1)}k`;
  return String(n);
}
