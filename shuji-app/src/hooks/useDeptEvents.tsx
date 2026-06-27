import { createContext, useContext, useEffect, useMemo, useState, type ReactNode } from 'react';
import { useTranslation } from 'react-i18next';
import { listen } from '@tauri-apps/api/event';
import { getActiveRoles } from '../api';
import type { DeptLogEntry, DeptStepEntry, RoundMetrics, RuntimeUpdate } from '../types';
import {
  normalizeDeptLabel,
  normalizeDeptLogEntry,
  normalizeDeptStepEntry,
} from '../utils/deptLog';
import {
  deriveLatestHumanSummary,
  deriveLatestStepByDept,
  deriveRecentHumanActions,
  type HumanAction,
} from '../utils/deptStepSummary';

// ── Types ────────────────────────────────────────────────────

interface DeptEventsState {
  /** 按部门聚合的最新日志条目 */
  latestLogs: Map<string, DeptLogEntry>;
  /** 完整日志列表（上限 200 条） */
  logEntries: DeptLogEntry[];
  /** 当前活跃部门列表 */
  activeDepts: string[];
  /** 实时 agent 步骤事件，按部门分桶 */
  deptSteps: Map<string, DeptStepEntry[]>;
  /** 各部门最新有意义步骤 */
  latestStepByDept: Map<string, DeptStepEntry>;
  /** 最近 1-4 条可读动作（跨部门） */
  recentHumanActions: HumanAction[];
  /** 当前最相关的可读动作（优先活跃部门） */
  latestHumanSummary: HumanAction | null;
  /** 本轮实时指标（由 runtime-update 推送） */
  roundMetrics: RoundMetrics | null;
}

interface DeptEventsContextValue extends DeptEventsState {
  /** 清空日志 */
  clearLogs: () => void;
}

// ── Constants ────────────────────────────────────────────────

/** 轮询兜底间隔 — 事件推送为主，轮询仅作低频备份 */
const ACTIVE_DEPT_POLL_MS = 10000;
const MAX_LOG_ENTRIES = 200;
const MAX_STEPS_PER_DEPT = 500;

// ── Context ──────────────────────────────────────────────────

const DeptEventsContext = createContext<DeptEventsContextValue>({
  latestLogs: new Map(),
  logEntries: [],
  activeDepts: [],
  deptSteps: new Map(),
  latestStepByDept: new Map(),
  recentHumanActions: [],
  latestHumanSummary: null,
  roundMetrics: null,
  clearLogs: () => {},
});

export function useDeptEvents() {
  return useContext(DeptEventsContext);
}

function applyRuntimeUpdate(
  update: RuntimeUpdate,
  setActiveDepts: (roles: string[]) => void,
  setRoundMetrics: (m: RoundMetrics | null) => void
) {
  setActiveDepts(update.active_roles.map((r) => normalizeDeptLabel(r)));
  if (update.round_metrics) {
    setRoundMetrics(update.round_metrics);
  }
}

// ── Provider ─────────────────────────────────────────────────

export function DeptEventsProvider({ children }: { children: ReactNode }) {
  const { i18n } = useTranslation();
  const lang = i18n.language?.startsWith('en') ? 'en' : 'zh';

  const [latestLogs, setLatestLogs] = useState<Map<string, DeptLogEntry>>(new Map());
  const [logEntries, setLogEntries] = useState<DeptLogEntry[]>([]);
  const [activeDepts, setActiveDepts] = useState<string[]>([]);
  const [deptSteps, setDeptSteps] = useState<Map<string, DeptStepEntry[]>>(new Map());
  const [roundMetrics, setRoundMetrics] = useState<RoundMetrics | null>(null);

  const latestStepByDept = useMemo(() => deriveLatestStepByDept(deptSteps), [deptSteps]);
  const recentHumanActions = useMemo(
    () => deriveRecentHumanActions(deptSteps, lang, 4),
    [deptSteps, lang]
  );
  const latestHumanSummary = useMemo(
    () => deriveLatestHumanSummary(deptSteps, activeDepts, lang),
    [deptSteps, activeDepts, lang]
  );

  // ── Centralized dept-log event listener ──
  useEffect(() => {
    const unlisten = listen<DeptLogEntry>('dept-log', (event) => {
      const entry = normalizeDeptLogEntry(event.payload);
      setLatestLogs((prev) => {
        const next = new Map(prev);
        next.set(entry.dept, entry);
        return next;
      });
      setLogEntries((prev) => {
        if (prev.length >= MAX_LOG_ENTRIES) {
          return [...prev.slice(1), entry];
        }
        return [...prev, entry];
      });
    });
    return () => {
      unlisten.then((f) => f());
    };
  }, []);

  // ── dept-step event listener (real-time agent steps) ──
  useEffect(() => {
    let cancelled = false;
    const setup = async () => {
      const unlisten = await listen<DeptStepEntry>('dept-step', (event) => {
        if (cancelled) return;
        const entry = normalizeDeptStepEntry(event.payload);
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

  // ── runtime-update event listener (active roles + round metrics) ──
  useEffect(() => {
    const unlisten = listen<RuntimeUpdate>('runtime-update', (event) => {
      applyRuntimeUpdate(event.payload, setActiveDepts, setRoundMetrics);
    });
    return () => {
      unlisten.then((f) => f());
    };
  }, []);

  // ── Fallback polling for active roles ──
  useEffect(() => {
    const poll = async () => {
      try {
        const roles = await getActiveRoles();
        setActiveDepts(roles.map((r) => normalizeDeptLabel(r)));
      } catch {
        // Silently retry on next tick
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
    <DeptEventsContext.Provider
      value={{
        latestLogs,
        logEntries,
        activeDepts,
        deptSteps,
        latestStepByDept,
        recentHumanActions,
        latestHumanSummary,
        roundMetrics,
        clearLogs,
      }}
    >
      {children}
    </DeptEventsContext.Provider>
  );
}
