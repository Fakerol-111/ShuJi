import { useEffect, useState } from "react";
import { getActiveRoles } from "../api";

const POLL_INTERVAL_MS = 1000;

export function useActiveDepts() {
  const [active, setActive] = useState<Set<string>>(new Set());

  useEffect(() => {
    let cancelled = false;

    const poll = async () => {
      try {
        const roles = await getActiveRoles();
        if (!cancelled) {
          setActive(new Set(roles));
        }
      } catch {
        // Ignore transient errors
      }
    };

    poll();
    const timer = window.setInterval(poll, POLL_INTERVAL_MS);
    return () => {
      cancelled = true;
      window.clearInterval(timer);
    };
  }, []);

  return active;
}
