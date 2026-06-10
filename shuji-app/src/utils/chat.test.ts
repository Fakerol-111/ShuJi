import { describe, it, expect } from 'vitest';
import { chatMessageKey, mergeMessages, initialCabinetMessage } from './chat';
import type { ChatMessage } from '../types';

function msg(
  overrides: Partial<ChatMessage> & { content: string; timestamp: string }
): ChatMessage {
  return {
    id: crypto.randomUUID(),
    role: '内阁',
    options: [],
    documents: [],
    ...overrides,
  };
}

describe('chatMessageKey', () => {
  it('returns the stable id when present', () => {
    const m = msg({ timestamp: 't1', role: '皇帝', content: 'hello' });
    const key = chatMessageKey(m);
    expect(key).toBe(m.id);
  });

  it('falls back to composite key when id is empty', () => {
    const m = msg({ timestamp: 't1', role: '皇帝', content: 'hello' });
    // Force empty id
    (m as unknown as Record<string, unknown>).id = '';
    const key = chatMessageKey(m);
    expect(key).toContain('t1');
    expect(key).toContain('皇帝');
    expect(key).toContain('hello');
  });
});

describe('mergeMessages', () => {
  it('returns prev unchanged when hist is empty', () => {
    const prev = [msg({ content: 'a', timestamp: 't1' })];
    const result = mergeMessages(prev, []);
    expect(result).toBe(prev); // reference equality when no change
  });

  it('returns prev unchanged when hist has same ids', () => {
    const prev = [msg({ id: 'id-1', content: 'hi', timestamp: 't1' })];
    const hist = [msg({ id: 'id-1', content: 'hi', timestamp: 't1' })];
    const result = mergeMessages(prev, hist);
    expect(result).toBe(prev);
  });

  it('appends new messages from hist', () => {
    const prev = [msg({ id: 'id-a', content: 'a', timestamp: 't1' })];
    const hist = [msg({ id: 'id-b', content: 'b', timestamp: 't2' })];
    const result = mergeMessages(prev, hist);
    expect(result).toHaveLength(2);
    expect(result[0].content).toBe('a');
    expect(result[1].content).toBe('b');
  });

  it('deduplicates by id', () => {
    const prev = [msg({ id: 'same-id', content: 'x'.repeat(100), timestamp: 't1' })];
    const hist = [msg({ id: 'same-id', content: 'x'.repeat(100), timestamp: 't1' })];
    const result = mergeMessages(prev, hist);
    expect(result).toHaveLength(1);
  });

  it('keeps both when ids differ even if content is same', () => {
    const prev = [msg({ id: 'id-1', content: 'same content', timestamp: 't1' })];
    const hist = [msg({ id: 'id-2', content: 'same content', timestamp: 't1' })];
    const result = mergeMessages(prev, hist);
    expect(result).toHaveLength(2);
  });

  it('handles mixed: some old, some new', () => {
    const prev = [
      msg({ id: 'old-1', content: 'old1', timestamp: 't1' }),
      msg({ id: 'old-2', content: 'old2', timestamp: 't2' }),
    ];
    const hist = [
      msg({ id: 'old-1', content: 'old1', timestamp: 't1' }),
      msg({ id: 'new-1', content: 'new1', timestamp: 't3' }),
    ];
    const result = mergeMessages(prev, hist);
    expect(result).toHaveLength(3);
    expect(result.map((m) => m.content)).toEqual(['old1', 'old2', 'new1']);
  });
});

describe('initialCabinetMessage', () => {
  it('creates a message with 内阁 role', () => {
    const m = initialCabinetMessage('测试');
    expect(m.role).toBe('内阁');
    expect(m.content).toBe('测试');
  });

  it('sets empty options and documents', () => {
    const m = initialCabinetMessage('hello');
    expect(m.options).toEqual([]);
    expect(m.documents).toEqual([]);
  });

  it('sets a valid ISO timestamp', () => {
    const m = initialCabinetMessage('x');
    expect(() => new Date(m.timestamp)).not.toThrow();
    expect(new Date(m.timestamp).toISOString()).toBe(m.timestamp);
  });

  it('generates a non-empty id', () => {
    const m = initialCabinetMessage('test');
    expect(m.id).toBeTruthy();
    expect(typeof m.id).toBe('string');
  });
});
