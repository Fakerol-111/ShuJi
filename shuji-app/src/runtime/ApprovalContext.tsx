/**
 * Approval Context — centralizes pending approvals state.
 *
 * Wraps usePendingApprovals so child components can consume
 * pendingApprovals and gateContext without prop drilling.
 *
 * Also provides a global `approvingDocId` lock so that multiple approval
 * UIs (top banner, artifact panel, doc preview) cannot approve the same
 * document simultaneously.
 */
import {
  createContext,
  useContext,
  useState,
  useEffect,
  useMemo,
  useCallback,
  type ReactNode,
} from 'react';
import { getPendingApprovals, getPipelineStatus, onProjectChanged } from '../api';
import type { PipelineRuntime } from '../types';
import { computeApprovalGateContext, type ApprovalGateContext } from '../utils/approvalGate';
import { useProjectContext } from './ProjectContext';
import { approveDocumentAndResume } from '../utils/approveDocument';

export interface ApprovalContextValue {
  pendingApprovals: string[];
  gateContext: ApprovalGateContext;
  pipeline: PipelineRuntime | null;
  /** The docId currently being approved, or null if no approval is in progress. */
  approvingDocId: string | null;
  /** Approve a document with global lock + optimistic removal from pendingApprovals. */
  approveDoc: (docId: string, comment?: string) => Promise<void>;
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
  approvingDocId: null,
  approveDoc: async () => {},
});

export function useApprovalContext() {
  return useContext(ApprovalContext);
}

export function ApprovalProvider({ children }: { children: ReactNode }) {
  const { project } = useProjectContext();
  const [pendingApprovals, setPendingApprovals] = useState<string[]>([]);
  const [pipeline, setPipeline] = useState<PipelineRuntime | null>(null);
  const [approvingDocId, setApprovingDocId] = useState<string | null>(null);

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

  /**
   * Global approve function with:
   * 1. Lock: sets `approvingDocId` so all approval UIs disable themselves
   * 2. Optimistic update: removes the doc from `pendingApprovals` immediately
   * 3. Idempotent: if `approvingDocId` is already set, returns immediately
   */
  const approveDoc = useCallback(
    async (docId: string, comment?: string) => {
      // Guard: if any approval is already in progress, refuse
      if (approvingDocId) return;
      setApprovingDocId(docId);
      // Optimistic: remove from pending list immediately so UIs disappear
      setPendingApprovals((prev) => prev.filter((id) => id !== docId));
      try {
        await approveDocumentAndResume(docId, comment);
      } finally {
        setApprovingDocId(null);
      }
    },
    [approvingDocId]
  );

  const value: ApprovalContextValue = {
    pendingApprovals,
    gateContext,
    pipeline,
    approvingDocId,
    approveDoc,
  };

  return <ApprovalContext.Provider value={value}>{children}</ApprovalContext.Provider>;
}
