/**
 * Centralized Tauri event subscription boundary.
 *
 * Every frontend event listener must go through this module instead of
 * importing `listen` from `@tauri-apps/api/event` directly. This gives us:
 *
 * 1. Single place to find all event-name strings.
 * 2. Runtime payload validation so malformed backend payloads don't
 *    corrupt React state.
 * 3. A clear API contract: changing a payload shape in Rust causes a
 *    TypeScript compile error here.
 */

import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import type {
  ChatMessage,
  ChatDeltaEvent,
  DeptLogEntry,
  DeptStepEntry,
  PlanInfo,
  Project,
  UsageUpdate,
  RuntimeUpdate,
} from '../types';

// ── Event name constants ──────────────────────────────────────────

export const TAURI_EVENTS = {
  chatMessage: 'chat-message',
  chatDelta: 'chat-delta',
  chatComplete: 'chat-complete',
  deptLog: 'dept-log',
  deptStep: 'dept-step',
  planUpdate: 'plan-update',
  projectUpdate: 'project-update',
  usageUpdate: 'usage-update',
  runtimeUpdate: 'runtime-update',
} as const;

// ── Payload type map ──────────────────────────────────────────────

export interface TauriEventPayloadMap {
  'chat-message': ChatMessage;
  'chat-delta': ChatDeltaEvent;
  'chat-complete': ChatMessage;
  'dept-log': DeptLogEntry;
  'dept-step': DeptStepEntry;
  'plan-update': PlanInfo;
  'project-update': Project;
  'usage-update': UsageUpdate;
  'runtime-update': RuntimeUpdate;
}

// ── Type guards ───────────────────────────────────────────────────

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

function isString(value: unknown): value is string {
  return typeof value === 'string';
}

function isArray(value: unknown): value is unknown[] {
  return Array.isArray(value);
}

export function isChatMessage(value: unknown): value is ChatMessage {
  return (
    isRecord(value) &&
    isString(value.id) &&
    isString(value.role) &&
    isString(value.content) &&
    isArray(value.options) &&
    isArray(value.documents) &&
    isString(value.timestamp)
  );
}

export function isChatDeltaEvent(value: unknown): value is ChatDeltaEvent {
  return (
    isRecord(value) && isString(value.message_id) && isString(value.role) && isString(value.delta)
  );
}

export function isDeptLogEntry(value: unknown): value is DeptLogEntry {
  return isRecord(value) && isString(value.dept) && isString(value.action) && isString(value.ts);
}

export function isDeptStepEntry(value: unknown): value is DeptStepEntry {
  return (
    isRecord(value) &&
    isString(value.dept) &&
    isString(value.ts) &&
    isRecord(value.kind) &&
    isString((value.kind as Record<string, unknown>).type)
  );
}

export function isPlanInfo(value: unknown): value is PlanInfo {
  return (
    isRecord(value) &&
    isArray(value.batches) &&
    typeof value.current === 'number' &&
    typeof value.complete === 'boolean'
  );
}

export function isProject(value: unknown): value is Project {
  return (
    isRecord(value) &&
    isString(value.id) &&
    isString(value.name) &&
    isString(value.goal) &&
    isString(value.working_dir)
  );
}

export function isUsageUpdate(value: unknown): value is UsageUpdate {
  return (
    isRecord(value) && isString(value.role) && (value.kind === 'token' || value.kind === 'context')
  );
}

export function isRuntimeUpdate(value: unknown): value is RuntimeUpdate {
  return (
    isRecord(value) &&
    isArray(value.active_roles) &&
    isString(value.trigger) &&
    isString(value.runtime_state)
  );
}

// ── Checked listener ──────────────────────────────────────────────

/**
 * Subscribe to a Tauri event with runtime payload validation.
 *
 * If the payload does not pass the `guard` function, the handler is NOT
 * called and a diagnostic is logged to the console (development only).
 */
function listenChecked<K extends keyof TauriEventPayloadMap>(
  eventName: K,
  guard: (value: unknown) => value is TauriEventPayloadMap[K],
  handler: (payload: TauriEventPayloadMap[K]) => void
): Promise<UnlistenFn> {
  return listen<TauriEventPayloadMap[K]>(eventName, (event) => {
    if (guard(event.payload)) {
      handler(event.payload);
    } else {
      console.warn(`[events] Dropped invalid payload for "${eventName}"`, event.payload);
    }
  });
}

// ── Typed subscription functions ──────────────────────────────────

export function onChatMessage(handler: (payload: ChatMessage) => void): Promise<UnlistenFn> {
  return listenChecked('chat-message', isChatMessage, handler);
}

export function onChatDelta(handler: (payload: ChatDeltaEvent) => void): Promise<UnlistenFn> {
  return listenChecked('chat-delta', isChatDeltaEvent, handler);
}

export function onChatComplete(handler: (payload: ChatMessage) => void): Promise<UnlistenFn> {
  return listenChecked('chat-complete', isChatMessage, handler);
}

export function onDeptLog(handler: (payload: DeptLogEntry) => void): Promise<UnlistenFn> {
  return listenChecked('dept-log', isDeptLogEntry, handler);
}

export function onDeptStep(handler: (payload: DeptStepEntry) => void): Promise<UnlistenFn> {
  return listenChecked('dept-step', isDeptStepEntry, handler);
}

export function onPlanUpdate(handler: (payload: PlanInfo) => void): Promise<UnlistenFn> {
  return listenChecked('plan-update', isPlanInfo, handler);
}

export function onProjectUpdate(handler: (payload: Project) => void): Promise<UnlistenFn> {
  return listenChecked('project-update', isProject, handler);
}

export function onUsageUpdate(handler: (payload: UsageUpdate) => void): Promise<UnlistenFn> {
  return listenChecked('usage-update', isUsageUpdate, handler);
}

export function onRuntimeUpdate(handler: (payload: RuntimeUpdate) => void): Promise<UnlistenFn> {
  return listenChecked('runtime-update', isRuntimeUpdate, handler);
}

// ── Semantic wrappers (refresh-only consumers) ────────────────────

/**
 * Subscribe to "project changed" — fires on `project-update` events.
 * The handler receives no payload; it is a refresh signal only.
 */
export function onProjectChanged(handler: () => void): Promise<UnlistenFn> {
  return listenChecked('project-update', isProject, () => handler());
}

/**
 * Subscribe to "usage data changed" — fires on `usage-update` events.
 * The handler receives no payload; it is a refresh signal only.
 */
export function onUsageChanged(handler: () => void): Promise<UnlistenFn> {
  return listenChecked('usage-update', isUsageUpdate, () => handler());
}

/**
 * Subscribe to "documents may have changed" — listens to chat-message,
 * dept-log, and plan-update internally. Useful for DocTree / DocPreview
 * refresh triggers that don't care about payload details.
 */
export function onDocsMayHaveChanged(handler: () => void): Promise<UnlistenFn>[] {
  return ['chat-message', 'dept-log', 'plan-update'].map((evt) => listen(evt, () => handler()));
}

// ── Re-export UnlistenFn for callers ──────────────────────────────

export type { UnlistenFn };
