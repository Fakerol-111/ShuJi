/**
 * Pending approvals hook — event-driven instead of polling.
 * Fetches on mount and on every `project-update` backend event.
 */
import { useState, useEffect } from 'react';
import { listen } from '@tauri-apps/api/event';
import { getPendingApprovals } from '../api';

export function usePendingApprovals(project: { working_dir?: string } | null) {
  const [pendingApprovals, setPendingApprovals] = useState<string[]>([]);

  useEffect(() => {
    if (!project) {
      setPendingApprovals([]);
      return;
    }
    const fetch = () => {
      getPendingApprovals()
        .then(setPendingApprovals)
        .catch(() => {});
    };
    // Initial fetch
    fetch();
    // Update on project-update events instead of polling every 3s
    const unlisten = listen('project-update', () => {
      fetch();
    });
    return () => {
      unlisten.then((f) => f());
    };
  }, [project?.working_dir, project]);

  return { pendingApprovals };
}
