import { useDeptEvents } from './useDeptEvents';
import type { DeptLogEntry } from '../types';

/** Tracks the most recent dept-log entry per department.
 *  Reads from centralized DeptEventsProvider instead of subscribing independently.
 */
export function useLatestDeptLogs(): Map<string, DeptLogEntry> {
  return useDeptEvents().latestLogs;
}
