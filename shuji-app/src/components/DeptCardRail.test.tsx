import { describe, it, expect, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import DeptCardRail from './DeptCardRail';

vi.mock('../hooks/useDeptEvents', () => ({
  useDeptEvents: () => ({ deptSteps: new Map() }),
}));

vi.mock('../constants', () => ({
  DEPT_META_LIST: [
    { label: '内阁', shortLabel: '内', color: '#6b4e9e', key: 'neige', icon: '内' },
    { label: '工部尚书', shortLabel: '工', color: '#a16207', key: 'gongbu', icon: '工' },
  ],
  DEPT_RAIL_GROUPS: [
    {
      title: '决策',
      titleEn: 'Decision',
      subtitle: '中枢',
      subtitleEn: 'Core',
      labels: ['内阁'],
    },
    {
      title: '执行',
      titleEn: 'Execution',
      subtitle: '六部',
      subtitleEn: 'Ministries',
      labels: ['工部尚书'],
    },
  ],
  getDeptMeta: (label: string) =>
    [
      { label: '内阁', shortLabel: '内', color: '#6b4e9e', key: 'neige', icon: '内' },
      { label: '工部尚书', shortLabel: '工', color: '#a16207', key: 'gongbu', icon: '工' },
    ].find((d) => d.label === label) || null,
  getDeptDisplayLabel: (meta: { label: string }) => meta.label,
}));

const baseProps = {
  selected: null,
  onSelect: vi.fn(),
  activeDepts: [],
  latestLogs: new Map(),
  planInfo: null,
  pinDept: false,
  onTogglePin: vi.fn(),
};

describe('DeptCardRail', () => {
  it('root container has overflow-hidden (not overflow-y-auto)', () => {
    const { container } = render(<DeptCardRail {...baseProps} />);
    const root = container.firstElementChild as HTMLElement;
    expect(root.className).toContain('overflow-hidden');
    // Should NOT have overflow-y-auto on root
    expect(root.className).not.toContain('overflow-y-auto');
  });

  it('scroll area wrapper has overflow-y-auto', () => {
    const { container } = render(<DeptCardRail {...baseProps} />);
    const root = container.firstElementChild as HTMLElement;
    const scrollArea = root.querySelector('.flex-1.min-h-0.overflow-y-auto');
    expect(scrollArea).toBeTruthy();
  });

  it('bottom control buttons are in shrink-0 container', () => {
    const { container } = render(<DeptCardRail {...baseProps} />);
    const root = container.firstElementChild as HTMLElement;
    const bottomControls = root.querySelector('.shrink-0.border-t');
    expect(bottomControls).toBeTruthy();
    // Verify both buttons exist
    const buttons = bottomControls!.querySelectorAll('button');
    expect(buttons.length).toBe(2);
  });

  it('renders department cards inside the scroll area', () => {
    render(<DeptCardRail {...baseProps} />);
    // Department names should be rendered by DeptCard
    // DeptCard mocks are not needed — we check the structure
    expect(screen.getByText('决策')).toBeTruthy();
    expect(screen.getByText('执行')).toBeTruthy();
  });

  it('has correct root flex column layout', () => {
    const { container } = render(<DeptCardRail {...baseProps} />);
    const root = container.firstElementChild as HTMLElement;
    expect(root.className).toContain('flex');
    expect(root.className).toContain('flex-col');
    expect(root.className).toContain('min-h-0');
    expect(root.className).toContain('shrink-0');
  });
});
