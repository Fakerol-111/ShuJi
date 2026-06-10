/**
 * Shared chat utilities — single source of truth for message merging and cabinet messages.
 *
 * Dedup uses the stable `id` field (UUID v4 from backend).
 */

import type { ChatMessage } from '../types';

/** Build a dedup key for a ChatMessage — uses stable id if available. */
export function chatMessageKey(m: ChatMessage): string {
  return m.id || `${m.timestamp}|${m.role}|${m.content.slice(0, 80)}`;
}

/**
 * Merge backend history into existing messages without duplicating.
 * Uses chatMessageKey for dedup (stable id when available). Returns a new array
 * or the original reference if no change.
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
    id: crypto.randomUUID(),
    role: '内阁',
    content,
    options: [],
    documents: [],
    timestamp: new Date().toISOString(),
  };
}
