import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import DutyBar from './DutyBar';

vi.mock('../hooks/useDeptEvents', () => ({
  useDeptEvents: () => ({
    activeDepts: ['gongbushangshu', 'bingbushangshu'],
    latestLogs: new Map([
      ['gongbushangshu', { dept: 'gongbushangshu', action: '→ 编码中' }],
      ['bingbushangshu', { dept: 'bingbushangshu', action: '→ 测试设计' }],
    ]),
  }),
}));

vi.mock('../hooks/useUsageStats', () => ({
  useUsageStats: () => ({
    tokenStats: { 'All Time': {} },
    roundMetrics: null,
  }),
}));

vi.mock('../hooks/useRunMetrics', () => ({
  useRunMetrics: () => null,
}));

vi.mock('../constants', () => ({
  getDeptMeta: (label: string) => {
    const map: Record<string, { color: string; key: string }> = {
      gongbushangshu: { color: '#a16207', key: 'gongbu' },
      bingbushangshu: { color: '#b83a3a', key: 'bingbu' },
    };
    return map[label] || null;
  },
  DEPT_META_LIST: [
    { label: 'gongbushangshu', color: '#a16207' },
    { label: 'bingbushangshu', color: '#b83a3a' },
  ],
  getDeptDisplayLabel: (_meta: { color: string }, _lang: string) => '工',
}));

vi.mock('./ValidationSummary', () => ({
  ValidationSummary: () => null,
}));

vi.mock('./DeptStatusPanel', () => ({
  default: () => <div data-testid="dept-status-panel" />,
}));

describe('DutyBar', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('root has overflow-hidden and min-w-0', () => {
    const { container } = render(<DutyBar />);
    const root = container.firstElementChild as HTMLElement;
    expect(root.className).toContain('overflow-hidden');
    expect(root.className).toContain('min-w-0');
  });

  it('department list container is flex-1 with overflow-x-auto', () => {
    const { container } = render(<DutyBar />);
    const root = container.firstElementChild as HTMLElement;
    const deptListContainer = root.querySelector('.flex-1.min-w-0.overflow-x-auto');
    expect(deptListContainer).toBeTruthy();
  });

  it('right-side stats and buttons are shrink-0', () => {
    const { container } = render(<DutyBar />);
    const root = container.firstElementChild as HTMLElement;
    const statsDiv = root.querySelector('.ml-auto.flex.items-center.shrink-0');
    expect(statsDiv).toBeTruthy();
  });

  it('multiple active depts render without overflow on root', () => {
    const { container } = render(<DutyBar />);
    const root = container.firstElementChild as HTMLElement;
    // Root should have overflow-hidden class
    expect(root.className).toContain('overflow-hidden');
    // Two dept tags should be rendered inside the scrollable area
    const deptListContainer = root.querySelector('.flex-1.min-w-0.overflow-x-auto');
    const deptTags = deptListContainer!.querySelectorAll('.inline-flex');
    expect(deptTags.length).toBe(2);
  });

  it('logs expanded panel uses max-h-48 (not fixed h-48)', async () => {
    const user = userEvent.setup();
    render(<DutyBar />);
    // The i18n key 'duty.logs' renders as '日志' in Chinese
    const logsBtn = screen.getByText('日志');
    await user.click(logsBtn);
    const panel = screen.getByTestId('dept-status-panel').parentElement;
    expect(panel!.className).toContain('max-h-48');
    // Verify it's max-h (capped), not fixed h-48
    expect(panel!.className.split(' ')).toContain('max-h-48');
    expect(panel!.className.split(' ')).not.toContain('h-48');
  });

  it('token expanded panel uses max-h-32', async () => {
    const user = userEvent.setup();
    render(<DutyBar />);
    // The i18n key 'duty.tokens' renders as '度支' in Chinese
    const tokenBtn = screen.getByText('度支');
    await user.click(tokenBtn);
    const panel = screen.getByText('缓存命中').closest('.max-h-32');
    expect(panel).toBeTruthy();
  });
});
