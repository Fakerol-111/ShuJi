/**
 * Approval Context — centralizes pending approvals state.
 *
 * Wraps usePendingApprovals so child components can consume
 * pendingApprovals and gateContext without prop drilling.
 */
import { createContext, useContext, useState, useEffect, useMemo, type ReactNode } from 'react';
import { getPendingApprovals, getPipelineStatus, onProjectChanged } from '../api';
import type { PipelineRuntime } from '../types';
import { computeApprovalGateContext, type ApprovalGateContext } from '../utils/approvalGate';
import { useProjectContext } from './ProjectContext';

export interface ApprovalContextValue {
  pendingApprovals: string[];
  gateContext: ApprovalGateContext;
  pipeline: PipelineRuntime | null;
}

const ApprovalContext = createContext<ApprovalContextValue>({
  pendingApprovals: [],
  gateContext: {
    active: false,
    docId: null,
    docType: '',
    stepId: null,
    stepLabel: null,
    stepAction: null,
    nextStepLabel: null,
    planSummary: null,
  },
  pipeline: null,
});

export function useApprovalContext() {
  return useContext(ApprovalContext);
}

export function ApprovalProvider({ children }: { children: ReactNode }) {
  const { project } = useProjectContext();
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

  const gateContext = useMemo(
    () => computeApprovalGateContext(pendingApprovals, pipeline),
    [pendingApprovals, pipeline]
  );

  const value: ApprovalContextValue = {
    pendingApprovals,
    gateContext,
    pipeline,
  };

  return <ApprovalContext.Provider value={value}>{children}</ApprovalContext.Provider>;
}
