import { useEffect, useMemo, useState } from 'react';
import { listen } from '@tauri-apps/api/event';
import { useTranslation } from 'react-i18next';
import { getPipelineStatus, getWorkflowGraph, getWorkflowState } from '../api';
import type {
  DeptLogEntry,
  PlanInfo,
  PipelineRuntime,
  RuntimeUpdate,
  WorkflowGraph,
  WorkflowState,
} from '../types';
import {
  buildRecentDocIds,
  buildTimelineNodes,
  computeNextAction,
} from '../utils/workflowTimeline';

const POLL_MS = 10000;

export interface UseWorkflowTimelineInput {
  activeDepts: string[];
  latestLogs: Map<string, DeptLogEntry>;
  pendingApprovals: string[];
  planInfo: PlanInfo | null;
}

export function useWorkflowTimeline({
  activeDepts,
  pendingApprovals,
  planInfo,
}: UseWorkflowTimelineInput) {
  const { t } = useTranslation();
  const [wfState, setWfState] = useState<WorkflowState | null>(null);
  const [graph, setGraph] = useState<WorkflowGraph | null>(null);
  const [pipeline, setPipeline] = useState<PipelineRuntime | null>(null);

  useEffect(() => {
    const load = () => {
      getWorkflowState()
        .then(setWfState)
        .catch(() => setWfState(null));
      getWorkflowGraph()
        .then(setGraph)
        .catch(() => setGraph(null));
      getPipelineStatus()
        .then(setPipeline)
        .catch(() => setPipeline(null));
    };
    load();
    const timer = window.setInterval(load, POLL_MS);
    const unlistenProject = listen('project-update', () => load());
    const unlistenRuntime = listen<RuntimeUpdate>('runtime-update', (event) => {
      const trigger = event.payload.trigger ?? '';
      if (trigger.startsWith('pipeline') || event.payload.pipeline) {
        getPipelineStatus()
          .then(setPipeline)
          .catch(() => setPipeline(null));
      }
    });
    return () => {
      window.clearInterval(timer);
      unlistenProject.then((f) => f());
      unlistenRuntime.then((f) => f());
    };
  }, []);

  const timelineNodes = useMemo(
    () => buildTimelineNodes(pipeline, graph, wfState, pendingApprovals),
    [pipeline, graph, wfState, pendingApprovals]
  );

  const recentDocIds = useMemo(
    () => buildRecentDocIds(pipeline, wfState, pendingApprovals),
    [pipeline, wfState, pendingApprovals]
  );

  const nextAction = useMemo(
    () =>
      computeNextAction(activeDepts, pendingApprovals, pipeline, {
        waitingApproval: (docId) => t('timeline.waitingApproval', { doc: docId }),
        waitingInput: (stepId) => t('timeline.waitingInput', { step: stepId }),
        waitingApprovalGate: t('timeline.waitingApprovalGate'),
        deptWorking: (dept) => t('timeline.deptWorking', { dept }),
        pipelineRunning: t('timeline.pipelineRunning'),
      }),
    [activeDepts, pendingApprovals, pipeline, t]
  );

  const pipelineProgress = useMemo(() => {
    if (!pipeline) return null;
    const total = pipeline.plan.steps.length;
    const done = pipeline.plan.steps.filter((s) => {
      const st = pipeline.step_status[s.step_id];
      return st === 'done' || st === 'skipped';
    }).length;
    return { done, total, summary: pipeline.plan.summary };
  }, [pipeline]);

  const gongbuBatch = useMemo(() => {
    if (!planInfo || planInfo.complete || planInfo.batches.length === 0) return null;
    const done = planInfo.batches.filter((b) => b.status === 'done').length;
    return { done, total: planInfo.batches.length };
  }, [planInfo]);

  const hasFlowActivity =
    timelineNodes.length > 0 ||
    pendingApprovals.length > 0 ||
    activeDepts.length > 0 ||
    pipeline !== null ||
    wfState !== null;

  return {
    wfState,
    graph,
    pipeline,
    timelineNodes,
    recentDocIds,
    nextAction,
    pipelineProgress,
    gongbuBatch,
    hasFlowActivity,
  };
}
