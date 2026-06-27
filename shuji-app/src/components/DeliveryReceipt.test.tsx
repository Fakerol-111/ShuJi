import { describe, it, expect, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import DeliveryReceipt from './DeliveryReceipt';
import type { ValidationReport } from '../types';

vi.mock('../api', () => ({
  listCheckpoints: vi
    .fn()
    .mockResolvedValue([
      { ts: '2026-01-01', role: '内阁', description: 'done', commit: 'abc12345' },
    ]),
}));

const passReport: ValidationReport = {
  ts: '2026-06-27',
  project_type: 'rust',
  overall_pass: true,
  checks: [{ name: 'tests', pass: true, summary: 'ok', details: {} }],
  ctrt_id: null,
};

describe('DeliveryReceipt', () => {
  it('renders receipt when validation passed and idle', async () => {
    render(
      <DeliveryReceipt
        validationReport={passReport}
        activeDeptsCount={0}
        recentDocIds={['rprt_001']}
      />
    );
    expect(await screen.findByText('交付收据')).toBeTruthy();
    expect(screen.getByText('rprt_001')).toBeTruthy();
  });

  it('hides while departments are active', () => {
    const { container } = render(
      <DeliveryReceipt validationReport={passReport} activeDeptsCount={2} />
    );
    expect(container.firstChild).toBeNull();
  });
});
