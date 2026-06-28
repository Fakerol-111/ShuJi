import { describe, it, expect } from 'vitest';
import { docIdToPath } from './docPath';

describe('docIdToPath', () => {
  it('returns path as-is when already a .shuji path', () => {
    expect(docIdToPath('.shuji/designs/dsgn-001.md')).toBe('.shuji/designs/dsgn-001.md');
  });

  it('maps revw_ prefix to reviews', () => {
    expect(docIdToPath('revw_001')).toBe('.shuji/reviews/revw_001.md');
  });

  it('maps plan_ prefix to designs', () => {
    expect(docIdToPath('plan_001')).toBe('.shuji/designs/plan_001.md');
  });

  it('maps reqs_ prefix to requirements', () => {
    expect(docIdToPath('reqs_001')).toBe('.shuji/requirements/reqs_001.md');
  });

  it('maps ddtl_ prefix to designs/detail', () => {
    expect(docIdToPath('ddtl_001')).toBe('.shuji/designs/detail/ddtl_001.md');
  });

  it('maps task_ prefix to tasks', () => {
    expect(docIdToPath('task_001')).toBe('.shuji/tasks/task_001.md');
  });

  it('maps ctrt_ prefix to contracts', () => {
    expect(docIdToPath('ctrt_001')).toBe('.shuji/contracts/ctrt_001.md');
  });

  it('maps rprt_ prefix to reports', () => {
    expect(docIdToPath('rprt_001')).toBe('.shuji/reports/rprt_001.md');
  });

  it('maps anls_ prefix to analysis', () => {
    expect(docIdToPath('anls_001')).toBe('.shuji/analysis/anls_001.md');
  });

  it('maps other ids to designs as default', () => {
    expect(docIdToPath('dsgn-001')).toBe('.shuji/designs/dsgn-001.md');
  });
});
