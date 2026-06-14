import { describe, it, expect } from 'vitest';

// Type for ValidationReport (mirrors Rust struct for frontend use)
interface CheckResult {
  name: string;
  pass: boolean;
  summary: string;
  details: Record<string, unknown>;
}

interface ValidationReport {
  ts: string;
  project_type: string;
  overall_pass: boolean;
  checks: CheckResult[];
  ctrt_id: string | null;
}

describe('ValidationReport parsing', () => {
  const sampleReport: ValidationReport = {
    ts: '2026-06-13T12:00:00Z',
    project_type: 'rust',
    overall_pass: true,
    checks: [
      {
        name: 'tests',
        pass: true,
        summary: 'all tests passed',
        details: { passed: 10, failed: 0 },
      },
    ],
    ctrt_id: null,
  };

  it('should parse valid report JSON', () => {
    const json = JSON.stringify(sampleReport);
    const parsed: ValidationReport = JSON.parse(json);
    expect(parsed.overall_pass).toBe(true);
    expect(parsed.project_type).toBe('rust');
    expect(parsed.checks).toHaveLength(1);
  });

  it('should detect overall pass from checks', () => {
    const allPass = sampleReport.checks.every((c) => c.pass);
    expect(allPass).toBe(true);
    expect(sampleReport.overall_pass).toBe(allPass);
  });

  it('should detect check failure', () => {
    const failing: ValidationReport = {
      ts: '2026-06-13T12:00:00Z',
      project_type: 'rust',
      overall_pass: false,
      checks: [
        { name: 'tests', pass: false, summary: 'test failed', details: { failed: 1 } },
      ],
      ctrt_id: null,
    };
    const allPass = failing.checks.every((c) => c.pass);
    expect(allPass).toBe(false);
    expect(failing.overall_pass).toBe(false);
  });

  it('should handle empty checks array', () => {
    const empty: ValidationReport = {
      ts: '',
      project_type: 'unknown',
      overall_pass: true,
      checks: [],
      ctrt_id: null,
    };
    expect(empty.checks).toHaveLength(0);
    expect(empty.overall_pass).toBe(true);
  });

  it('should preserve optional ctrt_id', () => {
    const withContract: ValidationReport = {
      ...sampleReport,
      ctrt_id: 'ctrt_001',
    };
    expect(withContract.ctrt_id).toBe('ctrt_001');
  });
});
