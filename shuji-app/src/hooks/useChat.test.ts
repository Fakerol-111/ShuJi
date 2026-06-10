import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { renderHook, act, waitFor } from '@testing-library/react';
import type { ChatMessage, RoleName } from '../types';
import { useChat } from './useChat';

// Mock Tauri event system
const mockListeners: Record<string, (payload: unknown) => void> = {};
const mockListen = vi.fn((event: string, cb: (payload: { payload: unknown }) => void) => {
  mockListeners[event] = (payload: unknown) => cb({ payload });
  return Promise.resolve(() => {
    delete mockListeners[event];
  });
});

vi.mock('@tauri-apps/api/event', () => ({
  listen: (event: string, cb: (payload: { payload: unknown }) => void) => mockListen(event, cb),
}));

// Mock API
const mockSendMessage = vi.fn();
const mockDiscussWithCabinet = vi.fn();
const mockGetChatHistory = vi.fn();
const mockCancelDiscuss = vi.fn().mockResolvedValue(undefined);

vi.mock('../api', () => ({
  sendMessage: (...args: unknown[]) => mockSendMessage(...args),
  discussWithCabinet: (...args: unknown[]) => mockDiscussWithCabinet(...args),
  getChatHistory: (...args: unknown[]) => mockGetChatHistory(...args),
  cancelDiscuss: (...args: unknown[]) => mockCancelDiscuss(...args),
}));

function createMsg(text: string, ts: string, role: RoleName = '内阁'): ChatMessage {
  return { role, content: text, options: [], documents: [], timestamp: ts };
}

describe('useChat', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockGetChatHistory.mockResolvedValue([]);
    // Clear any registered listeners
    Object.keys(mockListeners).forEach((k) => delete mockListeners[k]);
  });

  afterEach(() => {
    Object.keys(mockListeners).forEach((k) => delete mockListeners[k]);
  });

  it('initializes with empty messages', () => {
    const { result } = renderHook(() => useChat([]));
    expect(result.current.messages).toEqual([]);
  });

  it('initializes with initial messages', () => {
    const initial = [createMsg('hello', 't1')];
    const { result } = renderHook(() => useChat(initial));
    expect(result.current.messages).toHaveLength(1);
    expect(result.current.messages[0].content).toBe('hello');
  });

  it('initializes discussMsgs with a welcome message', () => {
    const { result } = renderHook(() => useChat([]));
    expect(result.current.discussMsgs).toHaveLength(1);
    expect(result.current.discussMsgs[0].content).toBe('想讨论什么？我随时可以聊。');
  });

  it('starts with decision tab', () => {
    const { result } = renderHook(() => useChat([]));
    expect(result.current.tab).toBe('decision');
  });

  it('optimistically adds emperor message on send, then marks failed on error', async () => {
    mockSendMessage.mockRejectedValue(new Error('API 错误'));

    const { result } = renderHook(() => useChat([]));
    expect(result.current.messages).toHaveLength(0);

    await act(async () => {
      await result.current.handleSend('测试消息');
    });

    // After send attempt: the optimistic message should be marked as failed
    expect(result.current.messages).toHaveLength(1);
    const msg = result.current.messages[0];
    expect(msg.role).toBe('皇帝');
    expect(msg.content).toBe('测试消息');
    expect(msg.status).toBe('failed');
  });

  it('optimistically adds emperor message on send, succeeds with no status', async () => {
    mockSendMessage.mockResolvedValue('ok');

    const { result } = renderHook(() => useChat([]));
    await act(async () => {
      await result.current.handleSend('成功消息');
    });

    expect(result.current.messages).toHaveLength(1);
    const msg = result.current.messages[0];
    expect(msg.role).toBe('皇帝');
    expect(msg.content).toBe('成功消息');
    // No status field means success
    expect(msg.status).toBeUndefined();
  });

  it('sets error message on send failure', async () => {
    mockSendMessage.mockRejectedValue(new Error('timeout'));
    const { result } = renderHook(() => useChat([]));
    await act(async () => {
      await result.current.handleSend('测试');
    });
    await waitFor(() => {
      expect(result.current.error).toBeTruthy();
    });
  });

  it('retrySend removes failed message and re-sends', async () => {
    const sendCalls: string[] = [];
    mockSendMessage.mockImplementation((text: string) => {
      sendCalls.push(text);
      return Promise.resolve('ok');
    });

    const { result } = renderHook(() => useChat([]));

    // First send fails
    mockSendMessage.mockRejectedValueOnce(new Error('error'));
    await act(async () => {
      await result.current.handleSend('重试消息');
    });

    // Now retry
    expect(result.current.messages).toHaveLength(1);
    expect(result.current.messages[0].status).toBe('failed');

    // Mock the second send to succeed
    mockSendMessage.mockResolvedValueOnce('ok');
    // Get the actual timestamp from the message
    const failedTs = result.current.messages[0].timestamp;
    await act(async () => {
      await result.current.retrySend('重试消息', failedTs);
    });

    // Old failed message removed, new one added
    expect(result.current.messages).toHaveLength(1);
    expect(result.current.messages[0].status).toBeUndefined();
  });

  it('appends chat-message events from Tauri', async () => {
    const { result } = renderHook(() => useChat([]));

    // Simulate Tauri chat-message event
    expect(mockListeners['chat-message']).toBeDefined();
    const deptMsg = createMsg('来自工部的回复', 't2', '工部尚书');

    await act(async () => {
      mockListeners['chat-message'](deptMsg);
    });

    expect(result.current.messages).toHaveLength(1);
    expect(result.current.messages[0].role).toBe('工部尚书');
    expect(result.current.messages[0].content).toBe('来自工部的回复');
  });

  it('updates planInfo from plan-update events', async () => {
    const { result } = renderHook(() => useChat([]));
    expect(mockListeners['plan-update']).toBeDefined();

    await act(async () => {
      mockListeners['plan-update']({
        batches: [{ name: 'Phase 1', goal: 'setup', status: 'done' }],
        current: 0,
        complete: false,
      });
    });

    expect(result.current.planInfo).not.toBeNull();
    expect(result.current.planInfo?.batches).toHaveLength(1);
  });

  it('sets planInfo to null when plan is complete', async () => {
    const { result } = renderHook(() => useChat([]));

    await act(async () => {
      mockListeners['plan-update']({
        batches: [],
        current: 0,
        complete: true,
      });
    });

    expect(result.current.planInfo).toBeNull();
  });

  it('handleDiscuss adds emperor message then reply', async () => {
    mockDiscussWithCabinet.mockResolvedValue(createMsg('同意你的方案', 't-reply', '内阁'));

    const { result } = renderHook(() => useChat([]));
    expect(result.current.discussMsgs).toHaveLength(1);
    expect(result.current.discussMsgs[0].content).toBe('想讨论什么？我随时可以聊。');

    await act(async () => {
      await result.current.handleDiscuss('我想扩大测试覆盖');
    });

    // Initial welcome + emperor message + cabinet reply = 3
    expect(result.current.discussMsgs).toHaveLength(3);
    expect(result.current.discussMsgs[1].role).toBe('皇帝');
    expect(result.current.discussMsgs[1].content).toBe('我想扩大测试覆盖');
    expect(result.current.discussMsgs[2].content).toBe('同意你的方案');
  });

  it('cancelDiscuss sets discussing to false and adds cancel message', async () => {
    // Keep the promise pending to keep discussing=true
    mockDiscussWithCabinet.mockReturnValue(new Promise(() => {}));

    const { result } = renderHook(() => useChat([]));

    await act(async () => {
      // Start discussing but promise never resolves
      result.current.handleDiscuss('测试讨论');
    });

    // Now cancel
    await act(async () => {
      result.current.cancelDiscuss();
    });

    expect(result.current.discussing).toBe(false);
    // Cancel message should be added
    const lastMsg = result.current.discussMsgs[result.current.discussMsgs.length - 1];
    expect(lastMsg.content).toContain('讨论已取消');
  });
});
