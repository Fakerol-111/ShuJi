/**
 * Shared chat utilities — single source of truth for message merging and cabinet messages.
 *
 * Since ChatMessage currently lacks a stable `id` field from the backend,
 * dedup uses a composite key: timestamp|role|content.length|content.slice(0, 80).
 * This is more reliable than the previous 40-char slice alone.
 * When the backend adds a stable `id`, switch to `msg.id` as the key.
 */

import type { ChatMessage } from '../types';

/** Build a composite dedup key for a ChatMessage (more reliable than raw timestamp alone). */
export function chatMessageKey(m: ChatMessage): string {
  return `${m.timestamp}|${m.role}|${m.content.length}|${m.content.slice(0, 80)}`;
}

/**
 * Merge backend history into existing messages without duplicating.
 * Uses chatMessageKey for dedup. Returns a new array or the original reference if no change.
 */
export function mergeMessages(prev: ChatMessage[], hist: ChatMessage[]): ChatMessage[] {
  if (hist.length === 0) return prev;
  const existing = new Set(prev.map(chatMessageKey));
  const newMsgs = hist.filter((m) => !existing.has(chatMessageKey(m)));
  return newMsgs.length > 0 ? [...prev, ...newMsgs] : prev;
}

/** Create a standard cabinet (内阁) welcome/system message. */
export function initialCabinetMessage(content: string): ChatMessage {
  return {
    role: '内阁',
    content,
    options: [],
    documents: [],
    timestamp: new Date().toISOString(),
  };
}
