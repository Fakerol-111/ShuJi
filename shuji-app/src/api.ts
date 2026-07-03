import { invoke } from '@tauri-apps/api/core';
import type {
  Project,
  ProjectSummary,
  ChatMessage,
  ProjectSnapshot,
  DeptLogEntry,
  AppConfig,
  ContextWindowConfig,
  TokenUsage,
  ContextStats,
  CheckpointEntry,
  RoundMetrics,
  LineageNode,
  TimelineData,
  DocDiffFile,
  TraceResult,
  DocumentLineRun,
  ImpactAnalysis,
  DocQuery,
  DocSummary,
  WorkflowGraph,
  PipelineRuntime,
  ApprovalConfig,
  PricingConfig,
  PricingEntry,
  ModelPrices,
  ReasoningConfig,
} from './types';

export interface ShujiEntry {
  name: string;
  path: string;
  type_label: string;
  is_dir: boolean;
  children: ShujiEntry[];
}

export interface ShujiDoc {
  content: string;
  path: string;
}

export async function createProject(
  name: string,
  goal: string,
  workingDir: string
): Promise<Project> {
  return invoke('create_project', { name, goal, workingDir });
}

export async function loadProject(workingDir: string): Promise<Project> {
  return invoke('load_project', { workingDir });
}

export async function getProject(): Promise<Project | null> {
  return invoke('get_project');
}

export async function listProjects(): Promise<ProjectSummary[]> {
  return invoke('list_projects');
}

// Send message to engine. Returns immediately (ack string);
// results come through chat-message events. Old return type preserved
// for compat — won't be used for state updates anymore.
export async function sendMessage(message: string): Promise<string> {
  return invoke('send_message', { message });
}

// Independent discussion with Cabinet — does not affect project state
export async function discussWithCabinet(message: string): Promise<ChatMessage> {
  return invoke('discuss_with_cabinet', { message });
}

/** Stream discuss reply — deltas arrive via `chat-delta`, final via `chat-complete`. */
export async function discussStream(message: string, messageId: string): Promise<void> {
  return invoke('discuss_stream', { message, messageId });
}

export async function cancelDiscuss(): Promise<void> {
  return invoke('cancel_discuss');
}

export async function getSnapshot(): Promise<ProjectSnapshot> {
  return invoke('get_snapshot');
}

export async function readDocument(subdir: string, filename: string): Promise<string | null> {
  return invoke('read_document', { subdir, filename });
}

export async function listDocuments(subdir: string): Promise<string[]> {
  return invoke('list_documents', { subdir });
}

export async function listShujiTree(projectDir: string): Promise<ShujiEntry[]> {
  return invoke('list_shuji_tree', { projectDir });
}

export async function readShujiDoc(projectDir: string, path: string): Promise<ShujiDoc> {
  return invoke('read_shuji_doc', { projectDir, path });
}

export interface DocumentDiff {
  diff: string;
  has_previous: boolean;
  added: number;
  removed: number;
}

export async function getDocumentDiff(projectDir: string, docPath: string): Promise<DocumentDiff> {
  return invoke('get_document_diff', { projectDir, docPath });
}

export async function listLogFiles(): Promise<string[]> {
  return invoke('list_log_files');
}

export async function readLogFile(filename: string): Promise<string[]> {
  return invoke('read_log_file', { filename });
}

export async function getRecentDirs(): Promise<string[]> {
  return invoke('get_recent_dirs');
}

export type { TokenUsage, ContextStats };
export async function getTokenStats(): Promise<Record<string, Record<string, TokenUsage>>> {
  return invoke('get_token_stats');
}

export type { PricingConfig, PricingEntry, ModelPrices };
export async function getPricing(): Promise<PricingConfig> {
  return invoke('get_pricing');
}
export async function savePricing(config: PricingConfig): Promise<void> {
  return invoke('save_pricing', { config });
}
export async function refreshPricing(): Promise<PricingConfig> {
  return invoke('refresh_pricing');
}

export async function getRoundMetrics(): Promise<RoundMetrics | null> {
  return invoke('get_round_metrics');
}

export async function getActiveRoles(): Promise<string[]> {
  return invoke('get_active_roles');
}

export async function getContextStats(): Promise<Record<string, ContextStats>> {
  return invoke('get_context_stats');
}

export async function compactContext(role: string): Promise<string> {
  return invoke('compact_context', { role });
}

export async function cancelProcessing(): Promise<void> {
  return invoke('cancel_processing');
}

export async function getChatHistory(): Promise<ChatMessage[]> {
  return invoke('get_chat_history');
}

export async function getDeptLogs(): Promise<DeptLogEntry[]> {
  return invoke('get_dept_logs');
}

export async function getConfig(): Promise<AppConfig> {
  return invoke('get_config');
}

export async function saveConfig(config: AppConfig): Promise<void> {
  return invoke('save_config', { config });
}

export async function setDotenvKey(key: string, value: string): Promise<void> {
  return invoke('set_dotenv_key', { key, value });
}

export async function getContextConfig(): Promise<ContextWindowConfig> {
  return invoke('get_context_config');
}

export async function saveContextConfig(config: ContextWindowConfig): Promise<void> {
  return invoke('save_context_config', { config });
}

export async function resetContextConfig(): Promise<void> {
  return invoke('reset_context_config');
}

// ── Model preset ───────────────────────────────────────────

export async function getModelPreset(): Promise<string> {
  return invoke('get_model_preset');
}

export async function setModelPreset(preset: string): Promise<void> {
  return invoke('set_model_preset', { preset });
}

// ── Approval mode ─────────────────────────────────────────

export type { ApprovalConfig, ApprovalMode } from './types';

export async function getApprovalConfig(): Promise<ApprovalConfig> {
  return invoke('get_approval_config');
}

export async function setApprovalConfig(config: ApprovalConfig): Promise<void> {
  return invoke('set_approval_config', { config });
}

// ── Pending approvals ─────────────────────────────────────

export async function getPendingApprovals(): Promise<string[]> {
  return invoke('get_pending_approvals');
}

// ── Reasoning config ─────────────────────────────────────────

export type { ReasoningConfig, ReasoningEffort, RoleReasoningConfig } from './types';

export async function getReasoningConfig(): Promise<ReasoningConfig> {
  return invoke('get_reasoning_config');
}

export async function setReasoningConfig(config: ReasoningConfig): Promise<void> {
  return invoke('set_reasoning_config', { config });
}

// ── Demo project ───────────────────────────────────────────

export async function createDemoProject(): Promise<Project> {
  return invoke('create_demo_project');
}

export async function resetDemoProject(): Promise<Project> {
  return invoke('reset_demo_project');
}

export async function runMockWorkflow(
  projectDir: string,
  scenario: string
): Promise<ChatMessage[]> {
  return invoke('run_mock_workflow', { projectDir, scenario });
}

// ── API health check ─────────────────────────────────────────

export async function checkApiConnection(
  apiKey: string,
  apiUrl: string,
  model: string
): Promise<string> {
  return invoke('check_api_connection', { apiKey, apiUrl, model });
}

// ── Workflow preset ─────────────────────────────────────────

export async function getWorkflowPreset(): Promise<string> {
  return invoke('get_workflow_preset');
}

export async function setWorkflowPreset(preset: string): Promise<void> {
  return invoke('set_workflow_preset', { preset });
}

// ── Workflow config (Intent × Governance) ───────────────────

// ── 文移图 ────────────────────────────────────────────────────

export async function getWorkflowGraph(): Promise<WorkflowGraph | null> {
  return invoke('get_workflow_graph');
}

export async function getPipelineStatus(): Promise<PipelineRuntime | null> {
  return invoke('get_pipeline_status');
}

export async function exportDiagnostics(): Promise<string> {
  return invoke('export_diagnostics');
}

export async function listWorkflowArchives(): Promise<[string, string][]> {
  return invoke('list_workflow_archives');
}

export async function loadWorkflowArchive(filename: string): Promise<WorkflowGraph | null> {
  return invoke('load_workflow_archive', { filename });
}

// ── Document approval ─────────────────────────────────────────

export async function setDocumentStatus(
  id: string,
  status: 'approved',
  emperorNote?: string
): Promise<string> {
  return invoke('set_document_status', { id, status, emperorNote });
}

// ── Soul management ────────────────────────────────────────────

export interface LearningEntry {
  id: string;
  role: string;
  scope: string;
  kind: string;
  content: string;
  evidence: string[];
  tags: string[];
  confidence: number;
  created_at: string;
  last_seen: string;
}

export interface LearningConfig {
  project_enabled: boolean;
  global_enabled: boolean;
  max_injected_chars_per_role: number;
  auto_extract: boolean;
  global_requires_approval: boolean;
}

export async function getSoulContent(role?: string, scope?: string): Promise<string> {
  return invoke('get_soul_content', { role, scope });
}

export async function clearSoul(role?: string, scope?: string): Promise<void> {
  return invoke('clear_soul', { role, scope });
}

export async function listSoulRoles(): Promise<string[]> {
  return invoke('list_soul_roles');
}

export async function listGlobalLearningCandidates(): Promise<LearningEntry[]> {
  return invoke('list_global_learning_candidates');
}

export async function approveGlobalLearning(candidateId: string): Promise<void> {
  return invoke('approve_global_learning', { candidateId });
}

export async function rejectGlobalLearning(candidateId: string): Promise<void> {
  return invoke('reject_global_learning', { candidateId });
}

export async function getLearningConfig(): Promise<LearningConfig> {
  return invoke('get_learning_config');
}

export async function setLearningGlobalEnabled(enabled: boolean): Promise<void> {
  return invoke('set_learning_global_enabled', { enabled });
}

// ── Checkpoint commands ──────────────────────────────────────

export async function listCheckpoints(
  role?: string,
  limit?: number,
  semanticOnly?: boolean
): Promise<CheckpointEntry[]> {
  return invoke('list_checkpoints', { role, limit, semanticOnly });
}

export async function restoreCheckpoint(commitHash: string): Promise<string> {
  return invoke('restore_checkpoint', { commitHash });
}

// ── Audit commands ───────────────────────────────────────────────

export async function getDocumentLineage(docId: string): Promise<LineageNode | null> {
  return invoke('get_document_lineage', { docId });
}

export async function getAuditTimeline(): Promise<TimelineData> {
  return invoke('get_audit_timeline');
}

export async function generateDeliveryReport(): Promise<string> {
  return invoke('generate_delivery_report');
}

export async function getDocumentDiffs(docId: string): Promise<DocDiffFile[]> {
  return invoke('get_document_diffs', { docId });
}

export async function readDocumentDiff(filename: string): Promise<string> {
  return invoke('read_document_diff', { filename });
}

export async function traceDocument(docId: string): Promise<TraceResult> {
  return invoke('trace_document', { docId });
}

export async function getDocumentLineRun(runId?: string): Promise<DocumentLineRun | null> {
  return invoke('get_document_line_run', { runId });
}

export async function getDocumentLineForDoc(docId: string): Promise<DocumentLineRun | null> {
  return invoke('get_document_line_for_doc', { docId });
}

export async function listDocumentLineRuns(): Promise<string[]> {
  return invoke('list_document_line_runs');
}

export async function analyzeDocumentImpact(docId: string): Promise<ImpactAnalysis> {
  return invoke('analyze_document_impact', { docId });
}

export async function queryDocuments(filter: DocQuery): Promise<DocSummary[]> {
  return invoke('query_documents', { filter });
}

// ── ESAA: Audit trail verification ──────────────────────────────

export interface BrokenLink {
  seq: number;
  expected_prev_hash: string;
  actual_prev_hash: string;
}

export interface VerificationReport {
  total_entries: number;
  chain_intact: boolean;
  first_entry_hash: string;
  last_entry_hash: string;
  broken_links: BrokenLink[];
  first_tampered_seq: number | null;
  pre_chain_entries: number;
}

export async function verifyAuditTrail(): Promise<VerificationReport> {
  return invoke('verify_audit_trail');
}
import type { ValidationReport, RunMetrics, RunMetricsSummary } from './types';

export async function validateDelivery(
  projectDir: string,
  ctrtId?: string,
  runLint?: boolean
): Promise<ValidationReport> {
  return invoke('validate_delivery_cmd', { projectDir, ctrtId, runLint, runContractDiff: false });
}

export async function getLatestRunMetrics(projectDir: string): Promise<RunMetrics | null> {
  return invoke('get_latest_run_metrics', { projectDir });
}

export async function listRunMetrics(
  projectDir: string,
  limit?: number
): Promise<RunMetricsSummary[]> {
  return invoke('list_run_metrics', { projectDir, limit });
}

// ── External IDE ────────────────────────────────────────────────

export interface EditorConfig {
  editor: 'vscode' | 'cursor' | 'trae' | 'zed' | 'sublime' | 'jetbrains' | 'custom';
  custom_command?: string | null;
  reuse_window: boolean;
}

export async function getEditorConfig(): Promise<EditorConfig> {
  return invoke('get_editor_config');
}

export async function setEditorConfig(config: EditorConfig): Promise<void> {
  return invoke('set_editor_config', { config });
}

export async function checkExternalEditor(config: EditorConfig): Promise<string> {
  return invoke('check_external_editor', { config });
}

export async function openInExternalEditor(
  projectDir: string,
  relPath: string,
  line?: number
): Promise<void> {
  return invoke('open_in_external_editor', { projectDir, relPath, line });
}

export async function openProjectInExternalEditor(projectDir: string): Promise<void> {
  return invoke('open_project_in_external_editor', { projectDir });
}

// ── Tauri event boundary (typed subscriptions) ────────────────────
// All frontend code must import Tauri event functions from here,
// NOT from `@tauri-apps/api/event` directly.
export {
  TAURI_EVENTS,
  onChatMessage,
  onChatDelta,
  onChatComplete,
  onDeptLog,
  onDeptStep,
  onPlanUpdate,
  onProjectUpdate,
  onUsageUpdate,
  onRuntimeUpdate,
  onProjectChanged,
  onUsageChanged,
  onDocsMayHaveChanged,
} from './api/events';
export type { UnlistenFn } from './api/events';
