import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import DeptActivityFeed from './DeptActivityFeed';

vi.mock('../hooks/useDeptEvents', () => ({
  useDeptEvents: vi.fn(),
}));

import { useDeptEvents } from '../hooks/useDeptEvents';

const mockUseDeptEvents = vi.mocked(useDeptEvents);

function entry(dept: string, action: string) {
  return { dept, action, ts: '14:32:05', detail: '' };
}

describe('DeptActivityFeed', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('shows empty state when no entries', () => {
    mockUseDeptEvents.mockReturnValue({
      logEntries: [],
      latestLogs: new Map(),
      activeDepts: [],
      clearLogs: vi.fn(),
    });
    render(<DeptActivityFeed />);
    expect(screen.getByText('暂无部门动态')).toBeTruthy();
  });

  it('renders department activity cards', () => {
    mockUseDeptEvents.mockReturnValue({
      logEntries: [entry('工部', '创建文档 .shuji/plan.md')],
      latestLogs: new Map(),
      activeDepts: ['工部'],
      clearLogs: vi.fn(),
    });
    render(<DeptActivityFeed />);
    expect(screen.getByText((c) => c.includes('创建文档'))).toBeTruthy();
  });

  it('calls onDocClick when doc link clicked', async () => {
    const onDocClick = vi.fn();
    mockUseDeptEvents.mockReturnValue({
      logEntries: [entry('工部', '创建文档 .shuji/plan.md')],
      latestLogs: new Map(),
      activeDepts: ['工部'],
      clearLogs: vi.fn(),
    });
    const user = userEvent.setup();
    render(<DeptActivityFeed onDocClick={onDocClick} />);
    const viewBtn = screen.getByText(/查看 →/);
    expect(viewBtn).toBeTruthy();
    await user.click(viewBtn);
    expect(onDocClick).toHaveBeenCalledWith('.shuji/plan.md');
  });

  it('deduplicates consecutive same dept+action', () => {
    mockUseDeptEvents.mockReturnValue({
      logEntries: [
        entry('工部', '创建文档 plan.md'),
        entry('工部', '创建文档 plan.md'),
        entry('中书令', '创建文档 dsgn.md'),
      ],
      latestLogs: new Map(),
      activeDepts: ['工部', '中书令'],
      clearLogs: vi.fn(),
    });
    const { container } = render(<DeptActivityFeed />);
    const cards = container.querySelectorAll('[class*="rounded-lg"]');
    expect(cards.length).toBe(2);
  });
});
