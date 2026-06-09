import { useState, useEffect } from "react";
import { listen } from "@tauri-apps/api/event";
import type { DeptLogEntry } from "../types";

/** Tracks the most recent dept-log entry per department.
 *  Provides a real-time view of "what is each department doing right now".
 */
export function useLatestDeptLogs(): Map<string, DeptLogEntry> {
  const [latest, setLatest] = useState<Map<string, DeptLogEntry>>(new Map());

  useEffect(() => {
    const unlisten = listen<DeptLogEntry>("dept-log", (event) => {
      const entry = event.payload;
      setLatest((prev) => {
        const next = new Map(prev);
        next.set(entry.dept, entry);
        return next;
      });
    });
    return () => {
      unlisten.then((f) => f());
    };
  }, []);

  return latest;
}
