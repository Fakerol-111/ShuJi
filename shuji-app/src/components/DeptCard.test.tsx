import { describe, it, expect } from 'vitest';
import { render, screen } from '@testing-library/react';
import DeptCard from './DeptCard';
import type { DeptMeta } from '../constants';

function mockMeta(overrides: Partial<DeptMeta> = {}): DeptMeta {
  return {
    key: 'gongbushangshu',
    label: '工部尚书',
    shortLabel: '工部',
    description: '编码实现',
    color: '#A16207',
    bg: 'bg-amber-50',
    text: 'text-amber-700',
    accent: 'border-l-amber-400',
    ...overrides,
  };
}

describe('DeptCard', () => {
  it('renders short label', () => {
    const meta = mockMeta();
    render(
      <DeptCard
        meta={meta}
        isActive={false}
        isSelected={false}
        hasError={false}
        latestAction=""
        onClick={() => {}}
      />
    );
    expect(screen.getByText('工部')).toBeTruthy();
  });

  it('renders latest action when provided', () => {
    render(
      <DeptCard
        meta={mockMeta()}
        isActive={false}
        isSelected={false}
        hasError={false}
        latestAction="read_file"
        onClick={() => {}}
      />
    );
    expect(screen.getByText('read_file')).toBeTruthy();
  });

  it('applies selected styles', () => {
    const { container } = render(
      <DeptCard
        meta={mockMeta()}
        isActive={false}
        isSelected={true}
        hasError={false}
        latestAction=""
        onClick={() => {}}
      />
    );
    const btn = container.querySelector('[aria-selected="true"]');
    expect(btn).toBeTruthy();
  });

  it('shows pulse class when active', () => {
    const { container } = render(
      <DeptCard
        meta={mockMeta()}
        isActive={true}
        isSelected={false}
        hasError={false}
        latestAction=""
        onClick={() => {}}
      />
    );
    const dot = container.querySelector('.animate-pulse');
    expect(dot).toBeTruthy();
  });

  it('shows error dot when hasError is true', () => {
    const { container } = render(
      <DeptCard
        meta={mockMeta()}
        isActive={false}
        isSelected={false}
        hasError={true}
        latestAction=""
        onClick={() => {}}
      />
    );
    const errDot = container.querySelector('.bg-vermillion');
    expect(errDot).toBeTruthy();
  });

  it('shows plan progress for 工部 with planInfo', () => {
    render(
      <DeptCard
        meta={mockMeta({ key: 'gongbushangshu' })}
        isActive={true}
        isSelected={false}
        hasError={false}
        latestAction=""
        planInfo={{
          batches: [
            { name: 'batch1', goal: '', status: 'done' },
            { name: 'batch2', goal: '', status: 'current' },
          ],
          current: 1,
          complete: false,
        }}
        onClick={() => {}}
      />
    );
    expect(screen.getByText('1/2')).toBeTruthy();
  });
});
