import { invoke } from "@tauri-apps/api/core";
import type { Project, ProjectSummary, ChatResponse, ProjectSnapshot } from "./types";

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

// Unified chat command — replaces step_workflow + make_decision
export async function sendMessage(message: string): Promise<ChatResponse> {
  return invoke("send_message", { message });
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

export async function listLogFiles(): Promise<string[]> {
  return invoke("list_log_files");
}

export async function readLogFile(filename: string): Promise<string[]> {
  return invoke("read_log_file", { filename });
}

export async function getRecentDirs(): Promise<string[]> {
  return invoke("get_recent_dirs");
}
