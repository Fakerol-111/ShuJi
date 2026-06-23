import { useMemo, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { useDeptEvents } from '../hooks/useDeptEvents';
import { useUsageStats } from '../hooks/useUsageStats';
import { getDeptMeta, DEPT_META_LIST } from '../constants';
import DeptStatusPanel from './DeptStatusPanel';

export default function DutyBar() {
  const { t } = useTranslation();
  const [logsExpanded, setLogsExpanded] = useState(false);
  const [tokenExpanded, setTokenExpanded] = useState(false);
  const { activeDepts, latestLogs } = useDeptEvents();
  const { tokenStats, roundMetrics } = useUsageStats();
  const [currency, setCurrency] = useState<'usd' | 'cny'>('usd');

  const deptArray =
    activeDepts.length > 0
      ? DEPT_META_LIST.filter((d) => activeDepts.includes(d.label)).map((d) => d.label)
      : [];

  const { tokenPrompt, tokenCached, tokenCompletion, tokenCost } = useMemo(() => {
    const roles = Object.values(tokenStats?.['All Time'] || {});
    const prompt = roles.reduce((sum, u) => sum + u.prompt_tokens, 0);
    const cached = roles.reduce((sum, u) => sum + (u.cached_prompt_tokens ?? 0), 0);
    const completion = roles.reduce((sum, u) => sum + u.completion_tokens, 0);
    const totalCost = roles.reduce(
      (sum, u) =>
        sum + (currency === 'cny' ? (u.estimated_cost_cny ?? 0) : (u.estimated_cost ?? 0)),
      0
    );
    return {
      tokenPrompt: prompt,
      tokenCached: cached,
      tokenCompletion: completion,
      tokenCost: totalCost > 0 ? totalCost.toFixed(3) : null,
    };
  }, [tokenStats, currency]);

  const hasActive = activeDepts.length > 0;
  const roundStarted = roundMetrics?.started_at ?? 0;
  const roundCompletion = roundMetrics?.completion_tokens ?? 0;

  return (
    <div className="shrink-0">
      <div className="h-7 bg-ink-900 border-t border-ink-800 flex items-center px-2 text-caption gap-0">
        <div className="flex items-center gap-1 min-w-0 shrink-0">
          <span className="text-gold/60 text-caption font-serif font-semibold tracking-wider mr-0.5">
            {t('duty.title')}
          </span>
          {deptArray.length === 0 ? (
            <span className="text-ink-500 italic text-caption">{t('activityBar.allQuiet')}</span>
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
                  className="inline-flex items-center gap-1 px-1.5 py-0.5 rounded text-caption font-serif max-w-[200px]"
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

        <div className="ml-auto flex items-center gap-2 text-caption font-mono text-ink-500 shrink-0">
          {hasActive && roundStarted > 0 && (
            <>
              <span className="text-gold/60">{t('duty.output')}</span>
              <span className="text-ink-300">{formatToken(roundCompletion)}</span>
            </>
          )}
          {tokenCost !== null && (
            <>
              <span className="text-ink-700">|</span>
              <span className="text-gold font-semibold">
                ≈ {currency === 'cny' ? '¥' : '$'}
                {tokenCost}
              </span>
            </>
          )}
        </div>

        <button
          onClick={() => setTokenExpanded((v) => !v)}
          className="ml-2 pl-2 border-l border-ink-800 flex items-center gap-1 text-caption text-ink-500 hover:text-ink-300 font-mono shrink-0"
          title={t('duty.tokens')}
        >
          <span>{tokenExpanded ? '▾' : '▸'}</span>
          {t('duty.tokens')}
        </button>

        <button
          onClick={() => setLogsExpanded((v) => !v)}
          className="ml-2 pl-2 border-l border-ink-800 flex items-center gap-1 text-caption text-ink-500 hover:text-ink-300 font-mono shrink-0"
        >
          <span>{logsExpanded ? '▾' : '▸'}</span>
          {t('duty.logs')}
        </button>
      </div>

      {tokenExpanded && (
        <div className="max-h-32 overflow-y-auto bg-ink-950 px-3 py-1.5 border-t border-ink-800">
          <div className="flex items-center gap-3 text-caption font-mono text-ink-500">
            <span className="text-jade/80">{t('duty.cacheHit')}</span>
            <span className="text-ink-300">{formatToken(tokenCached)}</span>
            <span className="text-ink-700">|</span>
            <span className="text-ink-400">{t('token.cacheMiss')}</span>
            <span className="text-ink-300">{formatToken(tokenPrompt - tokenCached)}</span>
            <span className="text-ink-700">|</span>
            <span className="text-gold/60">{t('duty.output')}</span>
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
                  className="text-caption text-ink-500 hover:text-ink-300 ml-0.5"
                  title="切换货币"
                >
                  {currency === 'usd' ? 'CNY' : 'USD'}
                </button>
              </>
            )}
          </div>
        </div>
      )}

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
