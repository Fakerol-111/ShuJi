export interface Project {
  id: string;
  name: string;
  goal: string;
  working_dir: string;
  overall: OverallStatus;
  phases: PhaseRuntime[];
  phase_count: number;
  created_at: string;
  updated_at: string;
}

export interface ProjectSummary {
  id: string;
  name: string;
  goal: string;
  working_dir: string;
  created_at: string;
  overall_status: string;
  phases_status: string;
}

export type OverallStatus =
  | 'NotStarted'
  | 'Designing'
  | 'Reviewing'
  | 'PendingApproval'
  | { Rejected: number }
  | 'Escalated'
  | 'Approved';

export interface PhaseRuntime {
  index: number;
  design: PhaseDesignStatus;
  execution: PhaseExecutionStatus;
}

export type PhaseDesignStatus =
  | 'NotStarted'
  | 'Designing'
  | 'Reviewing'
  | 'PendingApproval'
  | { Rejected: number }
  | 'Escalated'
  | 'Approved';

export type PhaseExecutionStatus =
  | 'NotStarted'
  | 'TaskBreakdown'
  | 'Testing'
  | 'Implementing'
  | 'Checking'
  | 'Standards'
  | 'Logging'
  | { Blocked: { reason: string } }
  | 'MinorIssue'
  | 'Completed';

export interface ProjectSnapshot {
  overall: OverallStatus;
  phases: PhaseSnapshot[];
  overall_progress: number;
}

export interface PhaseSnapshot {
  index: number;
  design: string;
  execution: string;
}

export interface Document {
  title: string;
  content: string;
  doc_type: string;
  path: string | null;
}

/** Metadata attached to chat messages for in-thread document cards. */
export interface ChatDocument {
  id: string;
  doc_type: string;
  title: string;
  status: string;
  path: string | null;
}

// ── Role names (union of all possible ChatMessage.role values) ──
/** Chinese role label used in ChatMessage.role and DeptMeta.label */
export type RoleName =
  | '皇帝'
  | '系统'
  | '内阁'
  | '中书令'
  | '门下侍中'
  | '尚书令'
  | '吏部尚书'
  | '兵部尚书'
  | '工部尚书'
  | '刑部尚书'
  | '礼部尚书';

// Chat types
export interface ChatMessage {
  id: string;
  role: RoleName;
  content: string;
  options: ChatOption[];
  documents: ChatDocument[];
  timestamp: string;
  /** Optional status for emperor messages: 'failed' on send error, undefined = sent OK */
  status?: 'failed';
}

export interface ChatOption {
  key: string;
  label: string;
  description: string;
}

export interface ChatResponse {
  messages: ChatMessage[];
  snapshot: ProjectSnapshot;
}

// Token usage stats
export interface TokenUsage {
  prompt_tokens: number;
  cached_prompt_tokens: number;
  uncached_prompt_tokens: number;
  completion_tokens: number;
  total_tokens: number;
  call_count: number;
  model?: string;
  estimated_cost?: number | null;
  estimated_cost_cny?: number | null;
}

export interface ModelPrices {
  input_per_m: number;
  output_per_m: number;
  cache_hit_input_per_m: number;
}

export interface PricingEntry {
  model_pattern: string;
  display_name: string;
  usd: ModelPrices;
  cny: ModelPrices | null;
}

export interface PricingConfig {
  usd_cny_rate: number;
  entries: PricingEntry[];
}

// Per-role context usage stats
export interface ContextStats {
  message_count: number;
  token_count: number;
  token_threshold: number;
  compressed: boolean;
  skill_count: number;
}

// API configuration
export interface RoleEndpoint {
  api_key: string;
  api_url: string;
  model: string;
}

export interface AppConfig {
  preset?: string;
  roles: Record<string, RoleEndpoint>;
}

// Real-time department status log (from dept-log Tauri event)
export interface DeptLogEntry {
  dept: string;
  action: string;
  ts: string;
  detail?: string;
}

// Context window configuration
export interface RoleContextConfig {
  token_threshold?: number;
  keep_recent_count?: number;
  mid_run_compact?: boolean;
}

export interface ContextWindowConfig {
  roles: Record<string, RoleContextConfig>;
}

// Checkpoint entry
export interface CheckpointEntry {
  ts: string;
  role: string;
  description: string;
  commit: string;
}

// 工部 plan progress (from plan-update Tauri event)
export interface PlanBatch {
  name: string;
  goal: string;
  status: 'done' | 'current' | 'pending';
}

export interface PlanInfo {
  batches: PlanBatch[];
  current: number;
  complete: boolean;
}

// Live round metrics (from get_round_metrics)
export type UsageUpdateKind = 'token' | 'context';

export interface UsageUpdate {
  role: string;
  kind: UsageUpdateKind;
}

export interface RoundMetrics {
  started_at: number;
  current_role: string;
  skill: string;
  prompt_tokens: number;
  cached_prompt_tokens: number;
  uncached_prompt_tokens: number;
  completion_tokens: number;
  total_tokens: number;
  dept_iterations: Record<string, number>;
}

/** Live runtime snapshot pushed via `runtime-update` Tauri event. */
export interface RuntimeUpdate {
  active_roles: string[];
  round_metrics: RoundMetrics | null;
  trigger: string;
}

// ── Department Inspector (九司卡片 + 部门观察器) ─────────────

export type SelectedDept = string | null;

// ── Real-time agent step events ─────────────────────────────

export interface DeptStepEntry {
  dept: string;
  ts: string;
  kind: DeptStepKind;
}

export type DeptStepKind =
  | { type: 'iteration'; n: number }
  | { type: 'thinking'; content: string }
  | { type: 'tool_call'; tool: string; args: Record<string, unknown> }
  | { type: 'tool_result'; tool: string; ok: boolean; summary: string }
  | { type: 'text'; content: string };

// ── Workflow Config (Intent × Governance) ───────────────────

export type Intent = 'auto' | 'greenfield_standard' | 'brownfield_optimize' | 'bugfix' | 'demo';
export type Governance = 'full' | 'standard' | 'fast' | 'audit';

export interface WorkflowConfig {
  intent: Intent;
  governance: Governance;
  intent_override: Intent | null;
}

export type ApprovalMode = 'manual' | 'auto';

export interface ApprovalConfig {
  mode: ApprovalMode;
  auto_retries: number;
}

// ── Workflow state (runtime) ──────────────────────────────

export interface WorkflowState {
  profile_id: string;
  governance: string;
  execution_chain_id: string;
  current_stage: string;
  artifacts: Record<string, string>;
}

export interface AuditEntry {
  ts: string;
  event: string;
  role: string;
  doc_id: string;
  detail: string;
}

export interface LineageNode {
  id: string;
  doc_type: string;
  author: string;
  timestamp: string;
  status: string;
  refs: number[];
  children: LineageNode[];
}

export interface TimelineSummary {
  total_events: number;
  by_event: [string, number][];
  by_role: [string, number][];
}

export interface TimelineData {
  entries: AuditEntry[];
  summary: TimelineSummary;
}

export interface DocDiffFile {
  filename: string;
  event: string;
  ts: string;
}

export interface ChainNode {
  id: string;
  doc_type: string;
  author: string;
  timestamp: string;
  stage: string;
  content_preview: string;
  direction: string;
}

export interface TraceResult {
  target: ChainNode | null;
  downstream: ChainNode[];
  upstream: ChainNode[];
}

export interface DocQuery {
  doc_type?: string[];
  author?: string;
  status?: string[];
  refs_id?: number;
  keyword?: string;
  since?: string;
  until?: string;
  limit?: number;
  offset?: number;
}

export interface DocSummary {
  id: string;
  doc_type: string;
  author: string;
  timestamp: string;
  status: string;
  refs: string;
  preview: string;
}

// ── 文移图 DAG ───────────────────────────────────────

export type GraphNodeStatus = 'active' | 'completed' | 'failed' | 'planned';

export interface GraphNode {
  id: number;
  role: string;
  instance: number;
  task_summary: string;
  status: GraphNodeStatus;
  created_at: string;
}

export interface GraphEdge {
  id: number;
  source: number;
  target: number;
  task_id: string;
  description: string;
  timestamp: string;
}

export interface WorkflowGraph {
  session_label: string;
  nodes: GraphNode[];
  edges: GraphEdge[];
}

// ── Pipeline runtime (from get_pipeline_status) ──────────────

export interface PipelinePlanStep {
  step_id: string;
  description: string;
  action: string;
  action_params: Record<string, unknown>;
  depends_on: string[];
  require_approval: boolean;
}

export interface PipelineRuntime {
  plan: {
    plan_id: string;
    summary: string;
    estimated_complexity: string;
    steps: PipelinePlanStep[];
  };
  step_status: Record<string, string>;
  current_step: string | null;
  artifacts: Record<string, string>;
  error_log: string[];
}

export type TimelineNodeStatus = 'done' | 'active' | 'pending' | 'failed' | 'waiting';

export interface TimelineNode {
  id: string;
  label: string;
  sublabel?: string;
  status: TimelineNodeStatus;
  dept?: string;
  docId?: string;
  kind: 'pipeline' | 'graph' | 'stage';
}

export type TimelineNextActionType = 'approval' | 'input' | 'running' | 'idle';

export interface TimelineNextAction {
  type: TimelineNextActionType;
  label: string;
  docId?: string;
  dept?: string;
}

// ── Validation Report (from Rust validate::report) ──────────────

export interface CheckResult {
  name: string;
  pass: boolean;
  summary: string;
  details: Record<string, unknown>;
}

export interface ValidationReport {
  ts: string;
  project_type: string;
  overall_pass: boolean;
  checks: CheckResult[];
  ctrt_id: string | null;
}

// ── Run Metrics (from Rust metrics::run) ────────────────────────

export interface RunMetrics {
  run_id: string;
  plan_id: string;
  started_at: string;
  completed_at: string | null;
  status: string;
  steps: StepMetric[];
  validation: ValidationReport | null;
}

export interface StepMetric {
  step_id: string;
  action: string;
  target: string | null;
  started_at: string;
  duration_ms: number;
  status: string;
  tool_errors: number;
  iterations: number;
}

export interface RunMetricsSummary {
  run_id: string;
  plan_id: string;
  started_at: string;
  status: string;
  step_count: number;
  overall_pass: boolean | null;
}
