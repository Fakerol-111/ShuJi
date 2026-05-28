import { useEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import type { DeptLogEntry } from "../types";

const ACTIVE_TIMEOUT_MS = 5000;

export function useActiveDepts() {
  const activeRef = useRef<Map<string, number>>(new Map());
  const [active, setActive] = useState<Set<string>>(new Set());

  useEffect(() => {
    const unlisten = listen<DeptLogEntry>("dept-log", (event) => {
      activeRef.current.set(event.payload.dept, Date.now());
      setActive(new Set(activeRef.current.keys()));
    });
    return () => { unlisten.then((f) => f()); };
  }, []);

  useEffect(() => {
    const timer = window.setInterval(() => {
      const now = Date.now();
      let changed = false;
      activeRef.current.forEach((time, dept) => {
        if (now - time > ACTIVE_TIMEOUT_MS) {
          activeRef.current.delete(dept);
          changed = true;
        }
      });
      if (changed) setActive(new Set(activeRef.current.keys()));
    }, 1000);
    return () => window.clearInterval(timer);
  }, []);

  return active;
}
