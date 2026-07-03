import type { DeptLogEntry, DeptStepEntry, RoundMetrics, RuntimeState } from '../types';
import type { HumanAction } from '../utils/deptStepSummary';

export interface RuntimeStateData {
  latestLogs: Map<string, DeptLogEntry>;
  logEntries: DeptLogEntry[];
  activeDepts: string[];
  deptSteps: Map<string, DeptStepEntry[]>;
  latestStepByDept: Map<string, DeptStepEntry>;
  recentHumanActions: HumanAction[];
  latestHumanSummary: HumanAction | null;
  roundMetrics: RoundMetrics | null;
  runtimeState: RuntimeState | null;
}

export interface RuntimeContextValue extends RuntimeStateData {
  clearLogs: () => void;
}
