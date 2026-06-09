/**
 * Pending approvals polling hook for ProjectDashboard.
 */
import { useState, useEffect } from 'react';
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
    fetch();
    const timer = setInterval(fetch, 3000);
    return () => clearInterval(timer);
  }, [project?.working_dir, project]);

  return { pendingApprovals };
}
