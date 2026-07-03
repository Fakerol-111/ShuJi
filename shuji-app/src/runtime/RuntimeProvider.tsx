import { createContext, useContext, useEffect, useMemo, useState, type ReactNode } from 'react';
import { useTranslation } from 'react-i18next';
import { onDeptLog, onDeptStep, onRuntimeUpdate, getActiveRoles } from '../api';
import type {
  DeptLogEntry,
  DeptStepEntry,
  RoundMetrics,
  RuntimeUpdate,
  RuntimeState,
} from '../types';
import {
  normalizeDeptLabel,
  normalizeDeptLogEntry,
  normalizeDeptStepEntry,
} from '../utils/deptLog';
import type { RuntimeContextValue } from './runtimeTypes';
import {
  selectLatestHumanSummary,
  selectLatestStepByDept,
  selectRecentHumanActions,
} from './runtimeSelectors';

const ACTIVE_DEPT_POLL_MS = 10000;
const MAX_LOG_ENTRIES = 200;
const MAX_STEPS_PER_DEPT = 500;

const RuntimeContext = createContext<RuntimeContextValue>({
  latestLogs: new Map(),
  logEntries: [],
  activeDepts: [],
  deptSteps: new Map(),
  latestStepByDept: new Map(),
  recentHumanActions: [],
  latestHumanSummary: null,
  roundMetrics: null,
  runtimeState: null,
  clearLogs: () => {},
});

export function useRuntime() {
  return useContext(RuntimeContext);
}

function applyRuntimeUpdate(
  update: RuntimeUpdate,
  setActiveDepts: (roles: string[]) => void,
  setRoundMetrics: (m: RoundMetrics | null) => void,
  setRuntimeState: (s: RuntimeState | null) => void
) {
  setActiveDepts(update.active_roles.map((r) => normalizeDeptLabel(r)));
  if (update.round_metrics) {
    setRoundMetrics(update.round_metrics);
  }
  setRuntimeState(update.runtime_state ?? null);
}

export function RuntimeProvider({ children }: { children: ReactNode }) {
  const { i18n } = useTranslation();
  const lang = i18n.language?.startsWith('en') ? 'en' : 'zh';

  const [latestLogs, setLatestLogs] = useState<Map<string, DeptLogEntry>>(new Map());
  const [logEntries, setLogEntries] = useState<DeptLogEntry[]>([]);
  const [activeDepts, setActiveDepts] = useState<string[]>([]);
  const [deptSteps, setDeptSteps] = useState<Map<string, DeptStepEntry[]>>(new Map());
  const [roundMetrics, setRoundMetrics] = useState<RoundMetrics | null>(null);
  const [runtimeState, setRuntimeState] = useState<RuntimeState | null>(null);

  const latestStepByDept = useMemo(() => selectLatestStepByDept(deptSteps), [deptSteps]);
  const recentHumanActions = useMemo(
    () => selectRecentHumanActions(deptSteps, lang),
    [deptSteps, lang]
  );
  const latestHumanSummary = useMemo(
    () => selectLatestHumanSummary(deptSteps, activeDepts, lang),
    [deptSteps, activeDepts, lang]
  );

  useEffect(() => {
    const unlisten = onDeptLog((event) => {
      const entry = normalizeDeptLogEntry(event);
      setLatestLogs((prev) => {
        const next = new Map(prev);
        next.set(entry.dept, entry);
        return next;
      });
      setLogEntries((prev) =>
        prev.length >= MAX_LOG_ENTRIES ? [...prev.slice(1), entry] : [...prev, entry]
      );
    });
    return () => {
      unlisten.then((f) => f());
    };
  }, []);

  useEffect(() => {
    let cancelled = false;
    const setup = async () => {
      const unlisten = await onDeptStep((event) => {
        if (cancelled) return;
        const entry = normalizeDeptStepEntry(event);
        setDeptSteps((prev) => {
          const next = new Map(prev);
          const dept = entry.dept;
          const steps = next.get(dept);
          const key = `${entry.ts}|${entry.kind.type}|${'tool' in entry.kind ? entry.kind.tool : ''}`;
          if (steps) {
            const last = steps[steps.length - 1];
            const lastKey = `${last.ts}|${last.kind.type}|${'tool' in last.kind ? last.kind.tool : ''}`;
            if (lastKey === key) return prev;
          }
          const nextSteps = steps ? [...steps, entry] : [entry];
          if (nextSteps.length > MAX_STEPS_PER_DEPT) {
            nextSteps.splice(0, nextSteps.length - MAX_STEPS_PER_DEPT);
          }
          next.set(dept, nextSteps);
          return next;
        });
      });
      if (cancelled) {
        unlisten();
      } else {
        return unlisten;
      }
    };
    const unlistenPromise = setup();
    return () => {
      cancelled = true;
      unlistenPromise.then((f) => f && f());
    };
  }, []);

  useEffect(() => {
    const unlisten = onRuntimeUpdate((payload) => {
      applyRuntimeUpdate(payload, setActiveDepts, setRoundMetrics, setRuntimeState);
    });
    return () => {
      unlisten.then((f) => f());
    };
  }, []);

  useEffect(() => {
    const poll = async () => {
      try {
        const roles = await getActiveRoles();
        setActiveDepts(roles.map((r) => normalizeDeptLabel(r)));
      } catch {
        // Event stream remains authoritative; polling retries on the next tick.
      }
    };
    poll();
    const timer = window.setInterval(poll, ACTIVE_DEPT_POLL_MS);
    return () => clearInterval(timer);
  }, []);

  const clearLogs = () => {
    setLatestLogs(new Map());
    setLogEntries([]);
    setDeptSteps(new Map());
  };

  return (
    <RuntimeContext.Provider
      value={{
        latestLogs,
        logEntries,
        activeDepts,
        deptSteps,
        latestStepByDept,
        recentHumanActions,
        latestHumanSummary,
        roundMetrics,
        runtimeState,
        clearLogs,
      }}
    >
      {children}
    </RuntimeContext.Provider>
  );
}
