import { useCallback, useEffect, useRef, useState } from 'react';
import { getLatestRunMetrics, validateDelivery } from '../api';
import type { ValidationReport } from '../types';

/**
 * Loads the latest validation report and re-runs validate_delivery when
 * active departments drop to zero after a run (task closure).
 */
export function useDeliveryValidation(projectDir: string | undefined, activeDepts: string[]) {
  const [report, setReport] = useState<ValidationReport | null>(null);
  const [loading, setLoading] = useState(false);
  const wasActiveRef = useRef(false);

  const loadCached = useCallback(async () => {
    if (!projectDir) return;
    try {
      const metrics = await getLatestRunMetrics(projectDir);
      if (metrics?.validation) {
        setReport(metrics.validation);
      }
    } catch {
      /* ignore */
    }
  }, [projectDir]);

  const refresh = useCallback(async () => {
    if (!projectDir) return;
    setLoading(true);
    try {
      const next = await validateDelivery(projectDir);
      setReport(next);
    } catch {
      /* keep prior report */
    } finally {
      setLoading(false);
    }
  }, [projectDir]);

  useEffect(() => {
    setReport(null);
    if (!projectDir) return;
    loadCached();
  }, [projectDir, loadCached]);

  useEffect(() => {
    if (activeDepts.length > 0) {
      wasActiveRef.current = true;
      return;
    }
    if (!projectDir || !wasActiveRef.current) return;
    wasActiveRef.current = false;
    refresh();
  }, [projectDir, activeDepts.length, refresh]);

  return { report, loading, refresh };
}
