import { useEffect, useState } from 'react';
import { getLatestRunMetrics } from '../api';
import type { RunMetrics } from '../types';

const POLL_MS = 5000;

/** Poll latest run metrics for DutyBar / status strip. */
export function useRunMetrics(projectDir: string | undefined) {
  const [metrics, setMetrics] = useState<RunMetrics | null>(null);

  useEffect(() => {
    if (!projectDir) {
      setMetrics(null);
      return;
    }
    const load = () => {
      getLatestRunMetrics(projectDir)
        .then(setMetrics)
        .catch(() => setMetrics(null));
    };
    load();
    const timer = window.setInterval(load, POLL_MS);
    return () => window.clearInterval(timer);
  }, [projectDir]);

  return metrics;
}
