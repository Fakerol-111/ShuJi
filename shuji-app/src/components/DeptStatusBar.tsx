import { useEffect, useState } from "react";
import { getTokenStats, getRoundMetrics } from "../api";
import { useActiveDepts } from "../hooks/useActiveDepts";
import { getDeptMeta, DEPT_META_LIST } from "../constants";
import type { RoundMetrics } from "../types";

const SKILL_LABELS: Record<string, string> = {
  workflow_standard: "标准", workflow_demo: "演示", workflow_simple: "简单",
  workflow_complex: "复杂", workflow_optimize: "优化", workflow_bugfix: "修复",
  workflow_refactor: "重构", workflow_audit: "审计", discuss: "廷议",
  summary: "奏报", clarify: "问对",
};

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
      // 停止计时：诸司无事时冻结时间
      const hasActive = DEPT_META_LIST.some((d) => activeSet.has(d.label) || activeSet.has(d.shortLabel));
      if (!hasActive) return;
      const secs = Math.floor((Date.now() - round.started_at) / 1000);
      if (secs < 60) setElapsed(`${secs}s`);
      else if (secs < 3600) setElapsed(`${Math.floor(secs / 60)}m${secs % 60}s`);
      else setElapsed(`${Math.floor(secs / 3600)}h${Math.floor((secs % 3600) / 60)}m`);
    };
    tick();
    const timer = window.setInterval(tick, 1000);
    return () => clearInterval(timer);
  }, [round, activeSet]);

  const activeDepts = DEPT_META_LIST.filter((d) => activeSet.has(d.label) || activeSet.has(d.shortLabel)).map((d) => d.label);

  return (
    <div className="h-7 bg-ink-900 border-t border-ink-800 shrink-0 flex items-center px-2 text-[11px] gap-0">
      {/* ── 值事 ── */}
      <div className="flex items-center gap-1 min-w-0 shrink-0">
        <span className="text-gold/60 text-[10px] font-serif font-semibold tracking-wider mr-0.5">值事</span>
        {activeDepts.length === 0 ? (
          <span className="text-ink-500 italic text-[10px]">诸司无事</span>
        ) : (
          activeDepts.map((dept) => {
            const meta = getDeptMeta(dept);
            const color = meta?.color || "#8B7355";
            return (
              <span
                key={dept}
                className="inline-flex items-center gap-1 px-1.5 py-0.5 rounded text-[10px] font-serif"
                style={{ backgroundColor: `${color}18` }}
              >
                <span className="w-1.5 h-1.5 rounded-full animate-pulse shrink-0" style={{ backgroundColor: color }} />
                <span className="text-ink-200 font-medium">{meta?.shortLabel || dept}</span>
              </span>
            );
          })
        )}
      </div>

      {/* ── 轮次信息 ── */}
      {round && round.started_at > 0 && (
        <div className="flex items-center gap-2 ml-2 pl-2 border-l border-ink-800 text-[10px] text-ink-400 font-mono shrink-0">
          {round.current_role && <span className="text-gold/80 font-semibold">{round.current_role}</span>}
          {round.skill && <span className="text-ink-500">· {SKILL_LABELS[round.skill] || round.skill}</span>}
          {elapsed && <span className="text-ink-500">· {elapsed}</span>}
        </div>
      )}

      {/* ── Token 计数 ── */}
      <div className="ml-auto flex items-center gap-2 text-[10px] font-mono text-ink-500 shrink-0">
        <span className="text-jade/80">输入缓存命中</span>
        <span className="text-ink-300">{formatToken(tokenCached)}</span>
        <span className="text-ink-700">|</span>
        <span className="text-ink-400">输入缓存未命中</span>
        <span className="text-ink-300">{formatToken(tokenPrompt - tokenCached)}</span>
        <span className="text-ink-700">|</span>
        <span className="text-gold/60">输出</span>
        <span className="text-ink-300">{formatToken(tokenCompletion)}</span>
      </div>
    </div>
  );
}

function formatToken(n: number) {
  if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(1)}M`;
  if (n >= 1000) return `${(n / 1000).toFixed(1)}k`;
  return String(n);
}
