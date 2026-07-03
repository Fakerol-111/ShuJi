/**
 * Pending approvals hook — event-driven instead of polling.
 * Fetches on mount and on every `project-update` backend event.
 * Also loads pipeline runtime for approval gate context (step / next step).
 */
import { useState, useEffect, useMemo } from 'react';
import { getPendingApprovals, getPipelineStatus, onProjectChanged } from '../api';
import type { PipelineRuntime } from '../types';
import { computeApprovalGateContext, type ApprovalGateContext } from '../utils/approvalGate';

export function usePendingApprovals(project: { working_dir?: string } | null) {
  const [pendingApprovals, setPendingApprovals] = useState<string[]>([]);
  const [pipeline, setPipeline] = useState<PipelineRuntime | null>(null);

  useEffect(() => {
    if (!project) {
      setPendingApprovals([]);
      setPipeline(null);
      return;
    }
    const fetch = () => {
      getPendingApprovals()
        .then(setPendingApprovals)
        .catch(() => setPendingApprovals([]));
      getPipelineStatus()
        .then(setPipeline)
        .catch(() => setPipeline(null));
    };
    fetch();
    const unlisten = onProjectChanged(() => {
      fetch();
    });
    return () => {
      unlisten.then((f) => f());
    };
  }, [project?.working_dir, project]);

  const gateContext: ApprovalGateContext = useMemo(
    () => computeApprovalGateContext(pendingApprovals, pipeline),
    [pendingApprovals, pipeline]
  );

  return { pendingApprovals, pipeline, gateContext };
}
