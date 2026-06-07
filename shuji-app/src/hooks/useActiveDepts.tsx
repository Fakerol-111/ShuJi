import { createContext, useContext, useEffect, useState, type ReactNode } from "react";
import { getActiveRoles } from "../api";

const POLL_INTERVAL_MS = 1000;

interface DeptActiveContextValue {
  active: Set<string>;
}

const DeptActiveContext = createContext<DeptActiveContextValue>({ active: new Set() });

let globalActive = new Set<string>();
let globalListeners: Array<(s: Set<string>) => void> = [];
let pollingStarted = false;

function startPolling() {
  if (pollingStarted) return;
  pollingStarted = true;

  const poll = async () => {
    try {
      const roles = await getActiveRoles();
      const newSet = new Set(roles);
      globalActive = newSet;
      for (const listener of globalListeners) {
        listener(newSet);
      }
    } catch {
      // Ignore transient errors
    }
  };

  poll();
  window.setInterval(poll, POLL_INTERVAL_MS);
}

export function DeptActiveProvider({ children }: { children: ReactNode }) {
  const [active, setActive] = useState<Set<string>>(globalActive);

  useEffect(() => {
    const listener = (s: Set<string>) => setActive(s);
    globalListeners.push(listener);
    startPolling();
    return () => {
      globalListeners = globalListeners.filter((l) => l !== listener);
    };
  }, []);

  return (
    <DeptActiveContext.Provider value={{ active }}>
      {children}
    </DeptActiveContext.Provider>
  );
}

export function useActiveDepts(): Set<string> {
  return useContext(DeptActiveContext).active;
}
