import { describe, it, expect } from 'vitest';
import { render, screen } from '@testing-library/react';
import { ValidationSummary } from './ValidationSummary';
import type { ValidationReport } from '../types';

const passReport: ValidationReport = {
  ts: '2026-06-13T12:00:00Z',
  project_type: 'rust',
  overall_pass: true,
  checks: [
    { name: 'tests', pass: true, summary: 'all tests passed', details: { passed: 10, failed: 0 } },
  ],
  ctrt_id: null,
};

const failReport: ValidationReport = {
  ts: '2026-06-13T12:00:00Z',
  project_type: 'rust',
  overall_pass: false,
  checks: [
    { name: 'tests', pass: false, summary: '2 tests failed', details: { failed: 2 } },
    { name: 'lint', pass: true, summary: 'lint clean', details: {} },
  ],
  ctrt_id: 'ctrt_001',
};

describe('ValidationSummary', () => {
  it('renders pass state', () => {
    render(<ValidationSummary report={passReport} />);
    expect(screen.getByText(/验证通过/i)).toBeDefined();
    expect(screen.getByText(/1\/1 项通过/i)).toBeDefined();
  });

  it('renders fail state with failed check names', () => {
    render(<ValidationSummary report={failReport} />);
    expect(screen.getByText(/未通过/i)).toBeDefined();
    expect(screen.getByText(/tests/)).toBeDefined();
  });

  it('renders loading state', () => {
    render(<ValidationSummary report={null} loading={true} />);
    expect(screen.getByText(/加载中/i)).toBeDefined();
  });

  it('renders empty state when no report and not loading', () => {
    render(<ValidationSummary report={null} />);
    expect(screen.getByText(/暂无验证/i)).toBeDefined();
  });
});
