import { useEffect, useState } from 'react';
import { getRoundMetrics } from '../api';
import type { RoundMetrics } from '../types';

interface WorkflowRibbonProps {
  totalStageCount: number;
  completedStageCount: number;
  pendingCount: number;
  onPendingClick?: () => void;
}

export default function WorkflowRibbon({
  totalStageCount,
  completedStageCount,
  pendingCount,
  onPendingClick,
}: WorkflowRibbonProps) {
  const [roundMetrics, setRoundMetrics] = useState<RoundMetrics | null>(null);
  const [elapsed, setElapsed] = useState('');

  useEffect(() => {
    const load = () => {
      getRoundMetrics()
        .then((m) => setRoundMetrics(m))
        .catch(() => {});
    };
    load();
    const timer = window.setInterval(load, 3000);
    return () => window.clearInterval(timer);
  }, []);

  useEffect(() => {
    if (!roundMetrics || roundMetrics.started_at <= 0) {
      setElapsed('');
      return;
    }
    const tick = () => {
      const secs = Math.floor((Date.now() - roundMetrics.started_at) / 1000);
      if (secs < 60) setElapsed(`${secs}s`);
      else if (secs < 3600) setElapsed(`${Math.floor(secs / 60)}m${secs % 60}s`);
      else setElapsed(`${Math.floor(secs / 3600)}h${Math.floor((secs % 3600) / 60)}m`);
    };
    tick();
    const timer = window.setInterval(tick, 1000);
    return () => clearInterval(timer);
  }, [roundMetrics]);

  const show = (totalStageCount ?? 0) > 0;
  if (!show && !roundMetrics) return null;

  return (
    <div className="shrink-0 flex items-center gap-2 px-3 py-1.5 bg-surface-elevated border-b border-fold">
      {(totalStageCount ?? 0) > 0 && (
        <div className="flex items-center gap-2 flex-1 min-w-0">
          <div className="flex-1 h-1.5 bg-ink-800 rounded-full overflow-hidden max-w-44">
            <div
              className="h-full bg-gold rounded-full transition-all duration-700"
              style={{ width: `${((completedStageCount ?? 0) / (totalStageCount ?? 1)) * 100}%` }}
            />
          </div>
          <span className="text-[10px] text-ink-400 whitespace-nowrap font-mono tabular-nums">
            {completedStageCount}/{totalStageCount}
          </span>
        </div>
      )}

      {roundMetrics?.current_role && (
        <span className="text-[10px] text-gold flex items-center gap-1 shrink-0">
          <span className="w-1.5 h-1.5 rounded-full bg-gold animate-pulse" />
          {roundMetrics.current_role}
        </span>
      )}

      {elapsed && <span className="text-[10px] text-ink-500 font-mono shrink-0">{elapsed}</span>}

      {pendingCount > 0 && (
        <button
          onClick={onPendingClick}
          className="flex items-center gap-1 px-2 py-0.5 rounded text-[10px] border border-vermillion/30 text-vermillion bg-vermillion/8 hover:bg-vermillion/15 transition-colors shrink-0"
        >
          <svg className="w-3 h-3" viewBox="0 0 24 24" fill="currentColor">
            <path d="M12 2L15.09 8.26L22 9.27L17 14.14L18.18 21.02L12 17.77L5.82 21.02L7 14.14L2 9.27L8.91 8.26L12 2Z" />
          </svg>
          朱批 {pendingCount}
        </button>
      )}
    </div>
  );
}
