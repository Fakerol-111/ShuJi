import type { DeptLogEntry, DeptStepEntry, RoundMetrics } from '../types';
import type { HumanAction } from '../utils/deptStepSummary';

export interface RuntimeState {
  latestLogs: Map<string, DeptLogEntry>;
  logEntries: DeptLogEntry[];
  activeDepts: string[];
  deptSteps: Map<string, DeptStepEntry[]>;
  latestStepByDept: Map<string, DeptStepEntry>;
  recentHumanActions: HumanAction[];
  latestHumanSummary: HumanAction | null;
  roundMetrics: RoundMetrics | null;
}

export interface RuntimeContextValue extends RuntimeState {
  clearLogs: () => void;
}
