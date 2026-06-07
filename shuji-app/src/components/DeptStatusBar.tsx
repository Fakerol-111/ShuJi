import { useEffect, useState } from "react";
import { getTokenStats, getRoundMetrics } from "../api";
import { useActiveDepts } from "../hooks/useActiveDepts";
import { DEPT_META, DEPT_ORDER } from "../constants";
import type { RoundMetrics } from "../types";

const SKILL_LABELS: Record<string, string> = {
  workflow_standard: "标准", workflow_demo: "演示", workflow_simple: "简单",
  workflow_complex: "复杂", workflow_optimize: "优化", workflow_bugfix: "修复",
  workflow_refactor: "重构", workflow_audit: "审计", discuss: "廷议",
  summary: "奏报", clarify: "问对",
};

/** 值事牌：六部当值看板。仿古制，显示当前在值诸司及进度。 */
export default function DeptStatusBar() {
  const activeSet = useActiveDepts();
  const [tokenPrompt, setTokenPrompt] = useState(0);
  const [tokenCached, setTokenCached] = useState(0);
  const [tokenCompletion, setTokenCompletion] = useState(0);
  const [round, setRound] = useState<RoundMetrics | null>(null);
  const [elapsed, setElapsed] = useState("");

  useEffect(() => {
    const load = () => {
      getTokenStats().then((stats) => {
        const roles = Object.values(stats["汇总"] || {});
        setTokenPrompt(roles.reduce((sum: number, u: any) => sum + u.prompt_tokens, 0));
        setTokenCached(roles.reduce((sum: number, u: any) => sum + (u.cached_prompt_tokens ?? 0), 0));
        setTokenCompletion(roles.reduce((sum: number, u: any) => sum + u.completion_tokens, 0));
      }).catch(() => {});
    };
    load();
    const timer = window.setInterval(load, 30000);
    return () => window.clearInterval(timer);
  }, []);

  useEffect(() => {
    const load = () => {
      getRoundMetrics().then((m) => setRound(m)).catch(() => {});
    };
    load();
    const timer = window.setInterval(load, 3000);
    return () => window.clearInterval(timer);
  }, []);

  useEffect(() => {
    if (!round) { setElapsed(""); return; }
    const tick = () => {
      const secs = Math.floor((Date.now() - round.started_at) / 1000);
      if (secs < 60) setElapsed(`${secs}s`);
      else if (secs < 3600) setElapsed(`${Math.floor(secs / 60)}m${secs % 60}s`);
      else setElapsed(`${Math.floor(secs / 3600)}h${Math.floor((secs % 3600) / 60)}m`);
    };
    tick();
    const timer = window.setInterval(tick, 1000);
    return () => clearInterval(timer);
  }, [round]);

  const activeDepts = DEPT_ORDER.filter((d) => activeSet.has(d));
  const idleDepts = DEPT_ORDER.filter((d) => !activeSet.has(d));

  return (
    <div className="bg-ink-900 border-t border-gold/20 shrink-0">
      {/* ── 值事牌（上部）：在值部门 + 实时指标 ── */}
      <div className="flex items-stretch min-h-[32px]">
        {/* 左侧：值事牌匾 */}
        <div className="flex items-center gap-0.5 pl-2 pr-1 py-1 overflow-x-auto">
          <span className="text-caption font-semibold text-gold/70 tracking-wider mr-1 whitespace-nowrap font-serif">
            值事
          </span>
          {activeDepts.length === 0 && (
            <span className="text-caption text-ink-600 italic whitespace-nowrap">诸司无事</span>
          )}
          {activeDepts.map((dept) => {
            const meta = DEPT_META[dept];
            return (
              <DutyPlaque
                key={dept}
                label={meta?.shortLabel || dept}
                color={meta?.color || "#8B7355"}
                active={true}
              />
            );
          })}
          {idleDepts.slice(0, 3).map((dept) => {
            const meta = DEPT_META[dept];
            return (
              <DutyPlaque
                key={dept}
                label={meta?.shortLabel || dept}
                color={meta?.color || "#8B7355"}
                active={false}
              />
            );
          })}
        </div>

        {/* 中间：当前轮次信息 */}
        {round && round.started_at > 0 && (
          <div className="flex items-center gap-2 px-2 border-l border-ink-700/50 text-caption text-ink-400 font-mono shrink-0">
            {round.current_role && (
              <span className="text-gold/80 font-semibold">{round.current_role}</span>
            )}
            {round.skill && (
              <span className="text-ink-500 bg-ink-800/50 px-1.5 rounded text-[10px]">
                {SKILL_LABELS[round.skill] || round.skill}
              </span>
            )}
            {elapsed && <span className="text-ink-500">{elapsed}</span>}
          </div>
        )}

        {/* 右侧：Token 计数 */}
        <div className="ml-auto flex items-center gap-2 px-2 text-[10px] font-mono text-ink-500 shrink-0">
          <span title={`输入缓存 ${formatToken(tokenCached)} / ${formatToken(tokenPrompt)}`}>
            <span className="text-jade/80">缓存</span> {formatToken(tokenCached)}
          </span>
          <span className="text-ink-700">·</span>
          <span title={`输出 ${formatToken(tokenCompletion)}`}>
            <span className="text-gold/60">出</span> {formatToken(tokenCompletion)}
          </span>
        </div>
      </div>
    </div>
  );
}

// ── 值事牌块 ─────────────────────────────────────────────
function DutyPlaque({ label, color, active }: { label: string; color: string; active: boolean }) {
  return (
    <div
      className={`relative flex items-center gap-1 px-2 py-0.5 rounded text-caption font-serif transition-all ${
        active
          ? "text-ink-50 font-semibold shadow-sm"
          : "text-ink-600/50"
      }`}
      style={{
        backgroundColor: active ? `${color}22` : "transparent",
        borderLeft: active ? `2px solid ${color}` : "2px solid transparent",
      }}
    >
      {active && (
        <span
          className="w-1.5 h-1.5 rounded-full animate-pulse shrink-0"
          style={{ backgroundColor: color }}
        />
      )}
      <span className={active ? "" : "line-through decoration-ink-700/30"}>{label}</span>
    </div>
  );
}

function formatToken(n: number) {
  if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(1)}M`;
  if (n >= 1000) return `${(n / 1000).toFixed(1)}k`;
  return String(n);
}
