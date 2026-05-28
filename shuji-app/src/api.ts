import { invoke } from "@tauri-apps/api/core";
import type { Project, ProjectSummary, ChatMessage, ProjectSnapshot, DeptLogEntry, AppConfig } from "./types";

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

export async function createProject(name: string, goal: string, workingDir: string): Promise<Project> {
  return invoke("create_project", { name, goal, workingDir });
}

export async function loadProject(workingDir: string): Promise<Project> {
  return invoke("load_project", { workingDir });
}

export async function getProject(): Promise<Project | null> {
  return invoke("get_project");
}

export async function listProjects(): Promise<ProjectSummary[]> {
  return invoke("list_projects");
}

// Send message to engine. Returns immediately (ack string);
// results come through chat-message events. Old return type preserved
// for compat — won't be used for state updates anymore.
export async function sendMessage(message: string): Promise<string> {
  return invoke("send_message", { message });
}

// Independent discussion with Cabinet — does not affect project state
export async function discussWithCabinet(message: string): Promise<ChatMessage> {
  return invoke("discuss_with_cabinet", { message });
}


export async function getSnapshot(): Promise<ProjectSnapshot> {
  return invoke("get_snapshot");
}

export async function readDocument(subdir: string, filename: string): Promise<string | null> {
  return invoke("read_document", { subdir, filename });
}

export async function listDocuments(subdir: string): Promise<string[]> {
  return invoke("list_documents", { subdir });
}

export async function listShujiTree(projectDir: string): Promise<ShujiEntry[]> {
  return invoke("list_shuji_tree", { projectDir });
}

export async function readShujiDoc(projectDir: string, path: string): Promise<ShujiDoc> {
  return invoke("read_shuji_doc", { projectDir, path });
}

export async function listLogFiles(): Promise<string[]> {
  return invoke("list_log_files");
}

export async function readLogFile(filename: string): Promise<string[]> {
  return invoke("read_log_file", { filename });
}

export async function getRecentDirs(): Promise<string[]> {
  return invoke("get_recent_dirs");
}

export interface TokenUsage {
  prompt_tokens: number;
  completion_tokens: number;
  total_tokens: number;
  call_count: number;
}

export async function getTokenStats(): Promise<Record<string, Record<string, TokenUsage>>> {
  return invoke("get_token_stats");
}

export async function cancelProcessing(): Promise<void> {
  return invoke("cancel_processing");
}

export async function getChatHistory(): Promise<ChatMessage[]> {
  return invoke("get_chat_history");
}

export async function getDeptLogs(): Promise<DeptLogEntry[]> {
  return invoke("get_dept_logs");
}

export async function getConfig(): Promise<AppConfig> {
  return invoke("get_config");
}

export async function saveConfig(config: AppConfig): Promise<void> {
  return invoke("save_config", { config });
}

export async function setDotenvKey(key: string, value: string): Promise<void> {
  return invoke("set_dotenv_key", { key, value });
}
