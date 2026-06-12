import { useEffect, useState } from 'react';
import { getTokenStats, getRoundMetrics } from '../api';
import { useDeptEvents } from '../hooks/useDeptEvents';
import { getDeptMeta, DEPT_META_LIST } from '../constants';
import DeptStatusPanel from './DeptStatusPanel';
import type { RoundMetrics, TokenUsage } from '../types';

const SKILL_LABELS: Record<string, string> = {
  workflow_standard: '标准',
  workflow_demo: '演示',
  workflow_simple: '简单',
  workflow_complex: '复杂',
  workflow_optimize: '优化',
  workflow_bugfix: '修复',
  workflow_refactor: '重构',
  workflow_audit: '审计',
  discuss: '廷议',
  summary: '奏报',
  clarify: '问对',
};

export default function DutyBar() {
  const [logsExpanded, setLogsExpanded] = useState(false);
  const { activeDepts, latestLogs } = useDeptEvents();
  const deptArray =
    activeDepts.length > 0
      ? DEPT_META_LIST.filter(
          (d) => activeDepts.includes(d.label) || activeDepts.includes(d.shortLabel)
        ).map((d) => d.label)
      : [];

  const [tokenPrompt, setTokenPrompt] = useState(0);
  const [tokenCached, setTokenCached] = useState(0);
  const [tokenCompletion, setTokenCompletion] = useState(0);
  const [tokenCost, setTokenCost] = useState<string | null>(null);
  const [currency, setCurrency] = useState<'usd' | 'cny'>('usd');
  const [round, setRound] = useState<RoundMetrics | null>(null);
  const [elapsed, setElapsed] = useState('');

  useEffect(() => {
    const load = () => {
      getTokenStats()
        .then((stats) => {
          const roles: TokenUsage[] = Object.values(stats['汇总'] || {});
          setTokenPrompt(roles.reduce((sum, u) => sum + u.prompt_tokens, 0));
          setTokenCached(roles.reduce((sum, u) => sum + (u.cached_prompt_tokens ?? 0), 0));
          setTokenCompletion(roles.reduce((sum, u) => sum + u.completion_tokens, 0));
          const totalCost = roles.reduce(
            (sum, u) =>
              sum + (currency === 'cny' ? (u.estimated_cost_cny ?? 0) : (u.estimated_cost ?? 0)),
            0
          );
          setTokenCost(totalCost > 0 ? totalCost.toFixed(3) : null);
        })
        .catch(() => {});
    };
    load();
    const timer = window.setInterval(load, 30000);
    return () => clearInterval(timer);
  }, [currency]);

  useEffect(() => {
    const load = () => {
      getRoundMetrics()
        .then((m) => setRound(m))
        .catch(() => {});
    };
    load();
    const timer = window.setInterval(load, 3000);
    return () => clearInterval(timer);
  }, []);

  useEffect(() => {
    if (!round || round.started_at <= 0) {
      setElapsed('');
      return;
    }
    const tick = () => {
      const hasActive = activeDepts.length > 0;
      if (!hasActive) return;
      const secs = Math.floor((Date.now() - round.started_at) / 1000);
      if (secs < 60) setElapsed(`${secs}s`);
      else if (secs < 3600) setElapsed(`${Math.floor(secs / 60)}m${secs % 60}s`);
      else setElapsed(`${Math.floor(secs / 3600)}h${Math.floor((secs % 3600) / 60)}m`);
    };
    tick();
    const timer = window.setInterval(tick, 1000);
    return () => clearInterval(timer);
  }, [round, activeDepts]);

  return (
    <div className="shrink-0">
      <div className="h-7 bg-ink-900 border-t border-ink-800 flex items-center px-2 text-[11px] gap-0">
        <div className="flex items-center gap-1 min-w-0 shrink-0">
          <span className="text-gold/60 text-[10px] font-serif font-semibold tracking-wider mr-0.5">
            值事
          </span>
          {deptArray.length === 0 ? (
            <span className="text-ink-500 italic text-[10px]">诸司无事</span>
          ) : (
            deptArray.map((dept) => {
              const meta = getDeptMeta(dept);
              const color = meta?.color || '#8B7355';
              const label = meta?.shortLabel || dept;
              const latestEntry = latestLogs.get(dept);
              const action = latestEntry
                ? latestEntry.action.replace(/^[❌→]\s*/, '').replace(/:.*/, '')
                : '';
              return (
                <span
                  key={dept}
                  className="inline-flex items-center gap-1 px-1.5 py-0.5 rounded text-[10px] font-serif max-w-[200px]"
                  style={{ backgroundColor: `${color}18` }}
                  title={action || label}
                >
                  <span
                    className="w-1.5 h-1.5 rounded-full animate-pulse shrink-0"
                    style={{ backgroundColor: color }}
                  />
                  <span className="text-ink-200 font-medium shrink-0">{label}</span>
                  {action && (
                    <span className="text-ink-400 truncate ml-0.5 max-w-[120px]">{action}</span>
                  )}
                </span>
              );
            })
          )}
        </div>

        {round && round.started_at > 0 && (
          <div className="flex items-center gap-2 ml-2 pl-2 border-l border-ink-800 text-[10px] text-ink-400 font-mono shrink-0">
            {round.current_role && (
              <span className="text-gold/80 font-semibold">{round.current_role}</span>
            )}
            {round.skill && (
              <span className="text-ink-500">· {SKILL_LABELS[round.skill] || round.skill}</span>
            )}
            {elapsed && <span className="text-ink-500">· {elapsed}</span>}
          </div>
        )}

        <div className="ml-auto flex items-center gap-2 text-[10px] font-mono text-ink-500 shrink-0">
          <span className="text-jade/80">输入缓存命中</span>
          <span className="text-ink-300">{formatToken(tokenCached)}</span>
          <span className="text-ink-700">|</span>
          <span className="text-ink-400">输入缓存未命中</span>
          <span className="text-ink-300">{formatToken(tokenPrompt - tokenCached)}</span>
          <span className="text-ink-700">|</span>
          <span className="text-gold/60">输出</span>
          <span className="text-ink-300">{formatToken(tokenCompletion)}</span>
          {tokenCost !== null && (
            <>
              <span className="text-ink-700">|</span>
              <span className="text-gold font-semibold">
                ≈ {currency === 'cny' ? '¥' : '$'}
                {tokenCost}
              </span>
              <button
                onClick={() => setCurrency(currency === 'usd' ? 'cny' : 'usd')}
                className="text-[9px] text-ink-500 hover:text-ink-300 ml-0.5"
                title="切换货币"
              >
                {currency === 'usd' ? 'CNY' : 'USD'}
              </button>
            </>
          )}
        </div>

        <button
          onClick={() => setLogsExpanded((v) => !v)}
          className="ml-2 pl-2 border-l border-ink-800 flex items-center gap-1 text-[10px] text-ink-500 hover:text-ink-300 font-mono shrink-0"
        >
          <span>{logsExpanded ? '▾' : '▸'}</span>
          日志
        </button>
      </div>

      {logsExpanded && (
        <div className="h-48 border-t border-ink-800 overflow-hidden">
          <DeptStatusPanel />
        </div>
      )}
    </div>
  );
}

function formatToken(n: number) {
  if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(1)}M`;
  if (n >= 1000) return `${(n / 1000).toFixed(1)}k`;
  return String(n);
}
