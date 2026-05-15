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
  | "NotStarted"
  | "Designing"
  | "Reviewing"
  | "PendingApproval"
  | { Rejected: number }
  | "Escalated"
  | "Approved";

export interface PhaseRuntime {
  index: number;
  design: PhaseDesignStatus;
  execution: PhaseExecutionStatus;
}

export type PhaseDesignStatus =
  | "NotStarted"
  | "Designing"
  | "Reviewing"
  | "PendingApproval"
  | { Rejected: number }
  | "Escalated"
  | "Approved";

export type PhaseExecutionStatus =
  | "NotStarted"
  | "TaskBreakdown"
  | "Testing"
  | "Implementing"
  | "Checking"
  | "Standards"
  | "Logging"
  | { Blocked: { reason: string } }
  | "MinorIssue"
  | "Completed";

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

// Chat types
export interface ChatMessage {
  role: string;
  content: string;
  options: ChatOption[];
  documents: Document[];
  timestamp: string;
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

// Real-time department status log (from dept-log Tauri event)
export interface DeptLogEntry {
  dept: string;
  action: string;
  ts: string;
  detail?: string;
}

// 工部 plan progress (from plan-update Tauri event)
export interface PlanBatch {
  name: string;
  goal: string;
  status: "done" | "current" | "pending";
}

export interface PlanInfo {
  batches: PlanBatch[];
  current: number;
  complete: boolean;
}
