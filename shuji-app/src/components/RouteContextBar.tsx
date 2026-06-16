import { useMemo } from 'react';
import { useTranslation } from 'react-i18next';
import { getDeptMeta } from '../constants';
import type { DeptLogEntry } from '../types';

const ROUTE_PREFIX = '→';

interface RouteContextBarProps {
  entries: DeptLogEntry[];
}

interface RouteSegment {
  from: string;
  to: string;
  color: string;
}

export default function RouteContextBar({ entries }: RouteContextBarProps) {
  const { t } = useTranslation();
  const chain = useMemo(() => {
    const routeEntries = entries.filter((e) => e.action.startsWith(ROUTE_PREFIX)).slice(-20);

    const seen = new Set<string>();
    const segments: RouteSegment[] = [];

    for (const entry of routeEntries) {
      const parts = entry.action.replace(ROUTE_PREFIX, '').trim();
      const meta = getDeptMeta(entry.dept);
      const color = meta?.color || '#8B7355';

      if (parts.includes('→')) {
        const [from, to] = parts.split('→').map((s) => s.trim());
        const key = `${from}→${to}`;
        if (!seen.has(key)) {
          seen.add(key);
          segments.push({ from, to, color });
        }
      } else {
        const key = `→${parts}`;
        if (!seen.has(key)) {
          seen.add(key);
          segments.push({ from: entry.dept, to: parts, color });
        }
      }
    }

    return segments;
  }, [entries]);

  if (chain.length === 0) return null;

  return (
    <div className="shrink-0 px-4 py-2 border-b border-fold bg-surface-paper/50">
      <div className="flex items-center gap-1.5 text-caption font-mono text-ink-500 flex-wrap">
        <span className="text-ink-400 font-semibold mr-0.5">{t('workflowGraph.routePath')}</span>
        {chain.map((seg, i) => (
          <span key={i} className="flex items-center gap-1">
            {i > 0 && <span className="text-ink-300 mx-0.5">→</span>}
            <span
              className="px-1 py-0.5 rounded"
              style={{ backgroundColor: `${seg.color}14`, color: seg.color }}
            >
              {seg.to || seg.from}
            </span>
          </span>
        ))}
      </div>
    </div>
  );
}
