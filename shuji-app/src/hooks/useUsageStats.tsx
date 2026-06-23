import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useRef,
  useState,
  type ReactNode,
} from 'react';
import { listen } from '@tauri-apps/api/event';
import { getContextStats, getRoundMetrics, getTokenStats } from '../api';
import type { ContextStats, RoundMetrics, TokenUsage } from '../types';

const COALESCE_MS = 80;

interface UsageStatsState {
  tokenStats: Record<string, Record<string, TokenUsage>> | null;
  contextStats: Record<string, ContextStats> | null;
  roundMetrics: RoundMetrics | null;
  loading: boolean;
  error: string;
  refresh: () => void;
  refreshTokenStats: () => Promise<void>;
  refreshContextStats: () => Promise<void>;
}

const UsageStatsContext = createContext<UsageStatsState>({
  tokenStats: null,
  contextStats: null,
  roundMetrics: null,
  loading: false,
  error: '',
  refresh: () => {},
  refreshTokenStats: async () => {},
  refreshContextStats: async () => {},
});

export function useUsageStats() {
  return useContext(UsageStatsContext);
}

/** Coalesce burst events (parallel agents) into a single async run. */
function createCoalescedRunner(run: () => Promise<void>, debounceMs: number) {
  let timer: ReturnType<typeof setTimeout> | null = null;
  let inFlight = false;
  let dirty = false;

  const execute = async () => {
    if (inFlight) {
      dirty = true;
      return;
    }
    inFlight = true;
    do {
      dirty = false;
      await run();
    } while (dirty);
    inFlight = false;
  };

  return () => {
    if (timer) clearTimeout(timer);
    timer = setTimeout(() => {
      timer = null;
      void execute();
    }, debounceMs);
  };
}

export function UsageStatsProvider({ children }: { children: ReactNode }) {
  const [tokenStats, setTokenStats] = useState<Record<string, Record<string, TokenUsage>> | null>(
    null
  );
  const [contextStats, setContextStats] = useState<Record<string, ContextStats> | null>(null);
  const [roundMetrics, setRoundMetrics] = useState<RoundMetrics | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState('');
  const mountedRef = useRef(true);

  useEffect(() => {
    mountedRef.current = true;
    return () => {
      mountedRef.current = false;
    };
  }, []);

  const fetchAll = useCallback(async () => {
    setLoading(true);
    try {
      const [tokens, context, round] = await Promise.all([
        getTokenStats(),
        getContextStats(),
        getRoundMetrics(),
      ]);
      if (!mountedRef.current) return;
      setTokenStats(tokens);
      setContextStats(context);
      setRoundMetrics(round);
      setError('');
    } catch (e) {
      if (!mountedRef.current) return;
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      if (mountedRef.current) setLoading(false);
    }
  }, []);

  const fetchTokenOnly = useCallback(async () => {
    try {
      const [tokens, round] = await Promise.all([getTokenStats(), getRoundMetrics()]);
      if (!mountedRef.current) return;
      setTokenStats(tokens);
      setRoundMetrics(round);
      setError('');
    } catch (e) {
      if (!mountedRef.current) return;
      setError(e instanceof Error ? e.message : String(e));
    }
  }, []);

  const fetchContextOnly = useCallback(async () => {
    try {
      const context = await getContextStats();
      if (!mountedRef.current) return;
      setContextStats(context);
      setError('');
    } catch (e) {
      if (!mountedRef.current) return;
      setError(e instanceof Error ? e.message : String(e));
    }
  }, []);

  const scheduleRef = useRef<(() => void) | null>(null);
  useEffect(() => {
    scheduleRef.current = createCoalescedRunner(fetchAll, COALESCE_MS);
  }, [fetchAll]);

  useEffect(() => {
    void fetchAll();
  }, [fetchAll]);

  useEffect(() => {
    const unlisten = listen('usage-update', () => {
      scheduleRef.current?.();
    });
    return () => {
      unlisten.then((fn) => fn());
    };
  }, []);

  return (
    <UsageStatsContext.Provider
      value={{
        tokenStats,
        contextStats,
        roundMetrics,
        loading,
        error,
        refresh: () => {
          void fetchAll();
        },
        refreshTokenStats: fetchTokenOnly,
        refreshContextStats: fetchContextOnly,
      }}
    >
      {children}
    </UsageStatsContext.Provider>
  );
}
