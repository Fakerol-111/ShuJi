import { getDeptMeta } from '../constants';
import type {
  GraphNode,
  PipelineRuntime,
  TimelineNextAction,
  TimelineNode,
  TimelineNodeStatus,
  WorkflowGraph,
  WorkflowState,
} from '../types';

function mapPipelineStepStatus(
  stepId: string,
  runtime: PipelineRuntime,
  pendingApprovals: string[]
): TimelineNodeStatus {
  const raw = runtime.step_status[stepId] || 'pending';
  const step = runtime.plan.steps.find((s) => s.step_id === stepId);

  if (raw === 'done' || raw === 'skipped') return 'done';
  if (raw === 'failed') return 'failed';

  if (step?.action === 'approval_gate' && pendingApprovals.length > 0) {
    return 'waiting';
  }

  if (stepId === runtime.current_step || raw === 'in_progress') {
    return 'active';
  }

  return 'pending';
}

function graphNodeStatus(status: GraphNode['status']): TimelineNodeStatus {
  switch (status) {
    case 'completed':
      return 'done';
    case 'failed':
      return 'failed';
    case 'active':
      return 'active';
    default:
      return 'pending';
  }
}

export function buildTimelineNodes(
  pipeline: PipelineRuntime | null,
  graph: WorkflowGraph | null,
  wfState: WorkflowState | null,
  pendingApprovals: string[]
): TimelineNode[] {
  if (pipeline && pipeline.plan.steps.length > 0) {
    return pipeline.plan.steps.map((step) => {
      const target =
        typeof step.action_params?.target === 'string' ? step.action_params.target : undefined;
      return {
        id: step.step_id,
        label: step.description || step.step_id,
        sublabel: step.action,
        status: mapPipelineStepStatus(step.step_id, pipeline, pendingApprovals),
        dept: target,
        docId: pipeline.artifacts[step.step_id],
        kind: 'pipeline',
      };
    });
  }

  if (graph && graph.nodes.length > 0) {
    return [...graph.nodes]
      .sort((a, b) => a.id - b.id)
      .map((node) => ({
        id: String(node.id),
        label: node.task_summary || node.role,
        sublabel: node.role,
        status: graphNodeStatus(node.status),
        dept: node.role,
        kind: 'graph' as const,
      }));
  }

  if (wfState && wfState.current_stage !== 'init') {
    return [
      {
        id: wfState.current_stage,
        label: wfState.current_stage,
        sublabel: wfState.profile_id,
        status: 'active',
        kind: 'stage',
      },
    ];
  }

  return [];
}

export function buildRecentDocIds(
  pipeline: PipelineRuntime | null,
  wfState: WorkflowState | null,
  pendingApprovals: string[],
  limit = 4
): string[] {
  const seen = new Set<string>();
  const ordered: string[] = [];

  const push = (id: string | undefined) => {
    if (!id || seen.has(id)) return;
    seen.add(id);
    ordered.push(id);
  };

  for (const id of pendingApprovals) push(id);
  if (pipeline) {
    for (const step of pipeline.plan.steps) {
      push(pipeline.artifacts[step.step_id]);
    }
  }
  if (wfState) {
    for (const id of Object.values(wfState.artifacts)) push(id);
  }

  return ordered.slice(0, limit);
}

export interface NextActionLabels {
  waitingApproval: (docId: string) => string;
  waitingInput: (stepId: string) => string;
  waitingApprovalGate: string;
  deptWorking: (deptLabel: string) => string;
  pipelineRunning: string;
}

export function computeNextAction(
  activeDepts: string[],
  pendingApprovals: string[],
  pipeline: PipelineRuntime | null,
  labels: NextActionLabels
): TimelineNextAction | null {
  if (pendingApprovals.length > 0) {
    const docId = pendingApprovals[0];
    return {
      type: 'approval',
      docId,
      label: labels.waitingApproval(docId),
    };
  }

  if (pipeline?.current_step) {
    const step = pipeline.plan.steps.find((s) => s.step_id === pipeline.current_step);
    if (step?.action === 'ask_user') {
      return {
        type: 'input',
        label: labels.waitingInput(step.step_id),
      };
    }
    if (step?.action === 'approval_gate') {
      return {
        type: 'approval',
        label: labels.waitingApprovalGate,
      };
    }
  }

  if (activeDepts.length > 0) {
    const dept = activeDepts[activeDepts.length - 1];
    const meta = getDeptMeta(dept);
    const deptLabel = meta?.shortLabel ?? dept;
    return {
      type: 'running',
      dept,
      label: labels.deptWorking(deptLabel),
    };
  }

  if (pipeline) {
    return {
      type: 'running',
      label: labels.pipelineRunning,
    };
  }

  return null;
}
