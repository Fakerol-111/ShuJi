import type { DeptStepEntry } from '../types';
import {
  deriveLatestHumanSummary,
  deriveLatestStepByDept,
  deriveRecentHumanActions,
} from '../utils/deptStepSummary';

export function selectLatestStepByDept(deptSteps: Map<string, DeptStepEntry[]>) {
  return deriveLatestStepByDept(deptSteps);
}

export function selectRecentHumanActions(
  deptSteps: Map<string, DeptStepEntry[]>,
  lang: 'zh' | 'en'
) {
  return deriveRecentHumanActions(deptSteps, lang, 4);
}

export function selectLatestHumanSummary(
  deptSteps: Map<string, DeptStepEntry[]>,
  activeDepts: string[],
  lang: 'zh' | 'en'
) {
  return deriveLatestHumanSummary(deptSteps, activeDepts, lang);
}
