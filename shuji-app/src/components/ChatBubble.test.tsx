import { describe, it, expect, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import ChatBubble from './ChatBubble';
import type { ChatMessage } from '../types';

// Mock react-markdown and its plugins (ESM compat in vitest/jsdom)
vi.mock('react-markdown', () => ({
  default: ({ children }: { children: string }) => <div data-testid="markdown">{children}</div>,
}));
vi.mock('remark-gfm', () => ({ default: () => {} }));
vi.mock('rehype-highlight', () => ({ default: () => {} }));

function emperorMsg(overrides: Partial<ChatMessage> = {}): ChatMessage {
  return {
    role: '皇帝',
    content: '准奏，按此执行。',
    options: [],
    documents: [],
    timestamp: '2026-01-01T00:00:00Z',
    ...overrides,
  };
}

function deptMsg(overrides: Partial<ChatMessage> = {}): ChatMessage {
  return {
    role: '工部尚书',
    content: '已完成第一阶段编码。',
    options: [],
    documents: [],
    timestamp: '2026-01-01T00:00:01Z',
    ...overrides,
  };
}

function msgWithOptions(): ChatMessage {
  return deptMsg({
    content: '请选择方案：',
    options: [
      { key: 'A', label: '快速方案', description: '最小改动，快速上线' },
      { key: 'B', label: '稳健方案', description: '更安全，但周期长' },
      { key: 'C', label: '补充御批', description: '我还有其他想法' },
    ],
  });
}

describe('ChatBubble', () => {
  it('renders emperor message on the right', () => {
    const { container } = render(<ChatBubble msg={emperorMsg()} onOption={() => {}} />);
    // Emperor message is in a flex container with justify-end
    const flexContainer = container.querySelector('.justify-end');
    expect(flexContainer).toBeTruthy();
    expect(screen.getByText('御')).toBeTruthy();
    expect(screen.getByText('准奏，按此执行。')).toBeTruthy();
  });

  it('renders department message on the left with role label', () => {
    render(<ChatBubble msg={deptMsg()} onOption={() => {}} />);
    expect(screen.getByText('工部尚书 回奏')).toBeTruthy();
    expect(screen.getByText('已完成第一阶段编码。')).toBeTruthy();
  });

  it('shows failed status with retry button for emperor messages', () => {
    const onRetry = vi.fn();
    render(
      <ChatBubble msg={emperorMsg({ status: 'failed' })} onOption={() => {}} onRetry={onRetry} />
    );
    expect(screen.getByText('发送失败')).toBeTruthy();
    const retryBtn = screen.getByText('重试');
    expect(retryBtn).toBeTruthy();
  });

  it('calls onRetry when retry button clicked', async () => {
    const onRetry = vi.fn();
    const user = userEvent.setup();
    render(
      <ChatBubble
        msg={emperorMsg({ status: 'failed', content: '测试消息', timestamp: 'ts-1' })}
        onOption={() => {}}
        onRetry={onRetry}
      />
    );
    await user.click(screen.getByText('重试'));
    expect(onRetry).toHaveBeenCalledWith('测试消息', 'ts-1');
  });

  it('renders options when present', () => {
    render(<ChatBubble msg={msgWithOptions()} onOption={() => {}} />);
    expect(screen.getByText('A. 快速方案')).toBeTruthy();
    expect(screen.getByText('B. 稳健方案')).toBeTruthy();
    expect(screen.getByText('C. 补充御批')).toBeTruthy();
  });

  it('shows description on option hover (via title)', () => {
    render(<ChatBubble msg={msgWithOptions()} onOption={() => {}} />);
    expect(screen.getByTitle('最小改动，快速上线')).toBeTruthy();
    expect(screen.getByTitle('更安全，但周期长')).toBeTruthy();
    expect(screen.getByTitle('我还有其他想法')).toBeTruthy();
  });

  it('directly calls onOption for non-supplement options (A, B)', async () => {
    const onOption = vi.fn();
    const user = userEvent.setup();
    render(<ChatBubble msg={msgWithOptions()} onOption={onOption} />);
    await user.click(screen.getByText('A. 快速方案'));
    expect(onOption).toHaveBeenCalledWith('A');
    await user.click(screen.getByText('B. 稳健方案'));
    expect(onOption).toHaveBeenCalledWith('B');
  });

  it('opens supplement textarea when clicking C (补充御批)', async () => {
    const onOption = vi.fn();
    const user = userEvent.setup();
    render(<ChatBubble msg={msgWithOptions()} onOption={onOption} />);
    await user.click(screen.getByText('C. 补充御批'));
    // Should show textarea + 遵旨 + 作罢 buttons
    expect(screen.getByPlaceholderText('在此补充御批...')).toBeTruthy();
    expect(screen.getByText('遵旨')).toBeTruthy();
    expect(screen.getByText('作罢')).toBeTruthy();
  });

  it('calls onOption with supplement text when 遵旨 clicked', async () => {
    const onOption = vi.fn();
    const user = userEvent.setup();
    render(<ChatBubble msg={msgWithOptions()} onOption={onOption} />);
    await user.click(screen.getByText('C. 补充御批'));
    await user.type(screen.getByPlaceholderText('在此补充御批...'), '务必在一个月内完成');
    await user.click(screen.getByText('遵旨'));
    expect(onOption).toHaveBeenCalledWith('C', '务必在一个月内完成');
  });

  it('closes supplement panel when 作罢 clicked', async () => {
    const onOption = vi.fn();
    const user = userEvent.setup();
    render(<ChatBubble msg={msgWithOptions()} onOption={onOption} />);
    await user.click(screen.getByText('C. 补充御批'));
    await user.click(screen.getByText('作罢'));
    // Back to option buttons
    expect(screen.getByText('A. 快速方案')).toBeTruthy();
    // Should NOT have called onOption
    expect(onOption).not.toHaveBeenCalled();
  });

  it('does not render options group when msg has no options', () => {
    const { container } = render(<ChatBubble msg={emperorMsg()} onOption={() => {}} />);
    // No option buttons should be present
    expect(container.querySelector('button')).toBeFalsy();
  });
});
