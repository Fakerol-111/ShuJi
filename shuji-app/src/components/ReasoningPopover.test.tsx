import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import ReasoningPopover from './ReasoningPopover';
import type { ReasoningConfig } from '../types';

const mockSetReasoningConfig = vi.fn().mockResolvedValue(undefined);
vi.mock('../api', () => ({
  getReasoningConfig: vi.fn(),
  setReasoningConfig: (...args: unknown[]) => mockSetReasoningConfig(...args),
}));

function mockConfig(overrides: Partial<ReasoningConfig> = {}): ReasoningConfig {
  return {
    enabled: true,
    effort: 'medium',
    budget_tokens: 0,
    roles: {},
    ...overrides,
  };
}

const anchorRect = new DOMRect(0, 0, 100, 30);

describe('ReasoningPopover', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('renders role label', () => {
    render(
      <ReasoningPopover
        roleKey="neige"
        roleLabel="内阁"
        config={mockConfig()}
        onClose={() => {}}
        anchorRect={anchorRect}
      />
    );
    expect(screen.getByText('内阁')).toBeTruthy();
  });

  it('shows thinking mode toggle', () => {
    render(
      <ReasoningPopover
        roleKey="neige"
        roleLabel="内阁"
        config={mockConfig()}
        onClose={() => {}}
        anchorRect={anchorRect}
      />
    );
    expect(screen.getByText('思考模式')).toBeTruthy();
  });

  it('shows effort chips when thinking is enabled', () => {
    render(
      <ReasoningPopover
        roleKey="neige"
        roleLabel="内阁"
        config={mockConfig({ enabled: true })}
        onClose={() => {}}
        anchorRect={anchorRect}
      />
    );
    expect(screen.getByText('轻量')).toBeTruthy();
    expect(screen.getByText('平衡')).toBeTruthy();
    expect(screen.getByText('深度')).toBeTruthy();
  });

  it('hides effort chips when thinking is disabled globally', () => {
    render(
      <ReasoningPopover
        roleKey="neige"
        roleLabel="内阁"
        config={mockConfig({ enabled: false })}
        onClose={() => {}}
        anchorRect={anchorRect}
      />
    );
    expect(screen.queryByText('轻量')).toBeNull();
    expect(screen.queryByText('平衡')).toBeNull();
    expect(screen.queryByText('深度')).toBeNull();
  });

  it('shows effort chips when role override has enabled=true even if global is false', () => {
    render(
      <ReasoningPopover
        roleKey="neige"
        roleLabel="内阁"
        config={mockConfig({ enabled: false, roles: { Neige: { enabled: true, effort: 'high' } } })}
        onClose={() => {}}
        anchorRect={anchorRect}
      />
    );
    expect(screen.getByText('轻量')).toBeTruthy();
    expect(screen.getByText('平衡')).toBeTruthy();
    expect(screen.getByText('深度')).toBeTruthy();
  });

  it('calls onClose on Escape key', () => {
    const onClose = vi.fn();
    render(
      <ReasoningPopover
        roleKey="neige"
        roleLabel="内阁"
        config={mockConfig()}
        onClose={onClose}
        anchorRect={anchorRect}
      />
    );
    fireEvent.keyDown(document, { key: 'Escape' });
    expect(onClose).toHaveBeenCalled();
  });

  it('shows description for effective effort', () => {
    render(
      <ReasoningPopover
        roleKey="neige"
        roleLabel="内阁"
        config={mockConfig({ roles: { Neige: { effort: 'high' } } })}
        onClose={() => {}}
        anchorRect={anchorRect}
      />
    );
    expect(screen.getByText(/深度推理/)).toBeTruthy();
  });

  it('shows disabled message when thinking is off', () => {
    render(
      <ReasoningPopover
        roleKey="neige"
        roleLabel="内阁"
        config={mockConfig({ enabled: false })}
        onClose={() => {}}
        anchorRect={anchorRect}
      />
    );
    expect(screen.getByText('该部门已关闭思考模式')).toBeTruthy();
  });

  it('calls onSaved with updated config after effort chip click', async () => {
    const onSaved = vi.fn();
    render(
      <ReasoningPopover
        roleKey="neige"
        roleLabel="内阁"
        config={mockConfig({ enabled: true, effort: 'medium' })}
        onClose={() => {}}
        anchorRect={anchorRect}
        onSaved={onSaved}
      />
    );
    fireEvent.click(screen.getByText('深度'));
    await vi.waitFor(() => {
      expect(onSaved).toHaveBeenCalled();
    });
    const savedConfig = onSaved.mock.calls[0][0] as ReasoningConfig;
    expect(savedConfig.roles['Neige']?.effort).toBe('high');
  });

  it('calls onSaved with enabled=false after toggle off', async () => {
    const onSaved = vi.fn();
    render(
      <ReasoningPopover
        roleKey="neige"
        roleLabel="内阁"
        config={mockConfig({ enabled: true })}
        onClose={() => {}}
        anchorRect={anchorRect}
        onSaved={onSaved}
      />
    );
    fireEvent.click(screen.getByText('思考模式'));
    await vi.waitFor(() => {
      expect(onSaved).toHaveBeenCalled();
    });
    const savedConfig = onSaved.mock.calls[0][0] as ReasoningConfig;
    expect(savedConfig.roles['Neige']?.enabled).toBe(false);
  });
});
