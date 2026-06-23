import { createContext, useContext, useEffect, useState, type ReactNode } from 'react';
import { listen } from '@tauri-apps/api/event';
import { getActiveRoles } from '../api';
import type { DeptLogEntry, DeptStepEntry } from '../types';
import {
  normalizeDeptLabel,
  normalizeDeptLogEntry,
  normalizeDeptStepEntry,
} from '../utils/deptLog';

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
}

interface DeptEventsContextValue extends DeptEventsState {
  /** 清空日志 */
  clearLogs: () => void;
}

// ── Constants ────────────────────────────────────────────────

const ACTIVE_DEPT_POLL_MS = 2000;
const MAX_LOG_ENTRIES = 200;
const MAX_STEPS_PER_DEPT = 500;

// ── Context ──────────────────────────────────────────────────

const DeptEventsContext = createContext<DeptEventsContextValue>({
  latestLogs: new Map(),
  logEntries: [],
  activeDepts: [],
  deptSteps: new Map(),
  clearLogs: () => {},
});

export function useDeptEvents() {
  return useContext(DeptEventsContext);
}

// ── Provider ─────────────────────────────────────────────────

export function DeptEventsProvider({ children }: { children: ReactNode }) {
  const [latestLogs, setLatestLogs] = useState<Map<string, DeptLogEntry>>(new Map());
  const [logEntries, setLogEntries] = useState<DeptLogEntry[]>([]);
  const [activeDepts, setActiveDepts] = useState<string[]>([]);
  const [deptSteps, setDeptSteps] = useState<Map<string, DeptStepEntry[]>>(new Map());

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
          // Dedup by ts + kind type + tool name
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

  // ── Unified active-dept polling ──
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
      value={{ latestLogs, logEntries, activeDepts, deptSteps, clearLogs }}
    >
      {children}
    </DeptEventsContext.Provider>
  );
}
