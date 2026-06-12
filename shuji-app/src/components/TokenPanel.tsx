import { useEffect, useState } from 'react';
import { getTokenStats, refreshPricing } from '../api';
import type { TokenUsage } from '../api';
import { formatError } from '../utils/error';
import { getDeptMeta, DEPT_ORDER } from '../constants';

type Currency = 'usd' | 'cny';

export default function TokenPanel() {
  const [stats, setStats] = useState<Record<string, Record<string, TokenUsage>> | null>(null);
  const [windowName, setWindowName] = useState('汇总');
  const [error, setError] = useState('');
  const [currency, setCurrency] = useState<Currency>('usd');
  const [refreshing, setRefreshing] = useState(false);

  const load = () => {
    setError('');
    getTokenStats()
      .then(setStats)
      .catch((e) => setError(formatError(e)));
  };

  const handleRefresh = async () => {
    setRefreshing(true);
    setError('');
    try {
      await refreshPricing();
      load();
    } catch (e) {
      setError(formatError(e));
    } finally {
      setRefreshing(false);
    }
  };

  useEffect(load, []);

  const current = stats?.[windowName] || {};
  const maxTotal = Math.max(...Object.values(current).map((u) => u.total_tokens), 1);

  const costLabel = currency === 'cny' ? '¥' : '$';
  const costKey = currency === 'cny' ? 'estimated_cost_cny' : 'estimated_cost';

  return (
    <div className="h-full overflow-y-auto p-3 bg-ink-50">
      <div className="flex items-center justify-between mb-3">
        <h3 className="text-xs font-bold text-ink-800">度支</h3>
        <div className="flex items-center gap-2">
          {/* Currency toggle */}
          <div className="flex text-[10px] border border-ink-300 rounded overflow-hidden">
            <button
              onClick={() => setCurrency('usd')}
              className={`px-1.5 py-0.5 ${currency === 'usd' ? 'bg-ink-900 text-white' : 'bg-ink-100 text-ink-500 hover:bg-ink-200'}`}
            >
              USD
            </button>
            <button
              onClick={() => setCurrency('cny')}
              className={`px-1.5 py-0.5 ${currency === 'cny' ? 'bg-ink-900 text-white' : 'bg-ink-100 text-ink-500 hover:bg-ink-200'}`}
            >
              CNY
            </button>
          </div>
          <button
            onClick={handleRefresh}
            disabled={refreshing}
            className="text-[10px] text-ink-400 hover:text-ink-700 disabled:opacity-50"
          >
            {refreshing ? '刷新中...' : '刷新价格'}
          </button>
        </div>
      </div>
      {stats && Object.keys(stats).length > 0 && (
        <div className="flex gap-1 mb-3 flex-wrap">
          {['今日', '近3日', '近7日', '汇总']
            .filter((w) => stats[w])
            .map((w) => (
              <button
                key={w}
                onClick={() => setWindowName(w)}
                className={`text-[10px] px-2 py-1 rounded ${windowName === w ? 'bg-ink-900 text-white' : 'bg-ink-100 text-ink-500 hover:bg-ink-200'}`}
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
          {Object.entries(current)
            .sort(([a], [b]) => roleOrder(a) - roleOrder(b))
            .map(([role, usage]) => {
              const pct = (usage.total_tokens / maxTotal) * 100;
              const cost = usage[costKey as keyof TokenUsage] as number | null | undefined;
              return (
                <div key={role}>
                  <div className="flex justify-between text-[11px] mb-1">
                    <span className="font-medium text-ink-700">
                      {getDeptMeta(role)?.shortLabel || role}
                    </span>
                    <span className="text-ink-500">
                      {usage.total_tokens.toLocaleString()}
                      {cost != null && (
                        <span className="text-gold ml-1">
                          ≈ {costLabel}
                          {cost.toFixed(3)}
                        </span>
                      )}
                    </span>
                  </div>
                  <div className="w-full bg-ink-200 rounded-full h-2 overflow-hidden">
                    <div
                      className="h-full rounded-full transition-all duration-500"
                      style={{
                        width: `${Math.max(pct, 2)}%`,
                        background: barColor(role),
                      }}
                    />
                  </div>
                  <div className="flex justify-between text-[9px] text-ink-400 mt-0.5">
                    <span>调用 {usage.call_count} 次</span>
                    <span>
                      缓存命中 {(usage.cached_prompt_tokens ?? 0).toLocaleString()} | 缓存未命中{' '}
                      {(usage.uncached_prompt_tokens ?? 0).toLocaleString()} | 输出{' '}
                      {usage.completion_tokens.toLocaleString()}
                    </span>
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
  const meta = getDeptMeta(role);
  if (!meta) return 999;
  const idx = DEPT_ORDER.indexOf(meta.label);
  return idx < 0 ? 999 : idx;
}

function barColor(role: string) {
  return getDeptMeta(role)?.color || '#6b7280';
}
