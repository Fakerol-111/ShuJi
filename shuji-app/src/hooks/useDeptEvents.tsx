import { createContext, useContext, useEffect, useState, type ReactNode } from 'react';
import { listen } from '@tauri-apps/api/event';
import { getActiveRoles } from '../api';
import type { DeptLogEntry } from '../types';

// ── Types ────────────────────────────────────────────────────

interface DeptEventsState {
  /** 按部门聚合的最新日志条目 */
  latestLogs: Map<string, DeptLogEntry>;
  /** 完整日志列表（上限 200 条） */
  logEntries: DeptLogEntry[];
  /** 当前活跃部门列表 */
  activeDepts: string[];
}

interface DeptEventsContextValue extends DeptEventsState {
  /** 清空日志 */
  clearLogs: () => void;
}

// ── Constants ────────────────────────────────────────────────

const ACTIVE_DEPT_POLL_MS = 2000;
const MAX_LOG_ENTRIES = 200;

// ── Context ──────────────────────────────────────────────────

const DeptEventsContext = createContext<DeptEventsContextValue>({
  latestLogs: new Map(),
  logEntries: [],
  activeDepts: [],
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

  // ── Centralized dept-log event listener ──
  useEffect(() => {
    const unlisten = listen<DeptLogEntry>('dept-log', (event) => {
      const entry = event.payload;
      // Update latest per-dept map
      setLatestLogs((prev) => {
        const next = new Map(prev);
        next.set(entry.dept, entry);
        return next;
      });
      // Append to full log list (capped)
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

  // ── Unified active-dept polling ──
  useEffect(() => {
    const poll = async () => {
      try {
        const roles = await getActiveRoles();
        setActiveDepts(roles);
      } catch {
        // Silently retry on next tick
      }
    };
    // Immediate first poll
    poll();
    const timer = window.setInterval(poll, ACTIVE_DEPT_POLL_MS);
    return () => clearInterval(timer);
  }, []);

  const clearLogs = () => {
    setLatestLogs(new Map());
    setLogEntries([]);
  };

  return (
    <DeptEventsContext.Provider value={{ latestLogs, logEntries, activeDepts, clearLogs }}>
      {children}
    </DeptEventsContext.Provider>
  );
}
