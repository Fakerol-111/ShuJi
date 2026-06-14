import { describe, it, expect } from 'vitest';
import { render, screen } from '@testing-library/react';
import RouteContextBar from './RouteContextBar';
import type { DeptLogEntry } from '../types';

function entry(overrides: Partial<DeptLogEntry> = {}): DeptLogEntry {
  return {
    dept: '尚书令',
    action: '→ 转交吏部',
    ts: '14:32:05',
    detail: '',
    ...overrides,
  };
}

describe('RouteContextBar', () => {
  it('renders nothing when no route entries', () => {
    const { container } = render(
      <RouteContextBar
        entries={[{ dept: '工部', action: '创建文档', ts: '14:32:05', detail: '' }]}
      />
    );
    expect(container.innerHTML).toBe('');
  });

  it('renders route segments from entries', () => {
    render(
      <RouteContextBar
        entries={[
          entry({ dept: '中书令', action: '→ 转交门下侍中', ts: '14:30:00' }),
          entry({ dept: '门下侍中', action: '→ 转交尚书令', ts: '14:31:00' }),
        ]}
      />
    );
    expect(screen.getByText('行文路径')).toBeTruthy();
    expect(screen.getByText('转交门下侍中')).toBeTruthy();
    expect(screen.getByText('转交尚书令')).toBeTruthy();
  });

  it('deduplicates consecutive same route segments', () => {
    const { container } = render(
      <RouteContextBar
        entries={[
          entry({ dept: '中书令', action: '→ 转交门下侍中', ts: '14:30:00' }),
          entry({ dept: '中书令', action: '→ 转交门下侍中', ts: '14:30:05' }),
        ]}
      />
    );
    const segments = container.querySelectorAll('[style*="background-color"]');
    expect(segments.length).toBe(1);
  });

  it('handles empty entries array', () => {
    const { container } = render(<RouteContextBar entries={[]} />);
    expect(container.innerHTML).toBe('');
  });
});
