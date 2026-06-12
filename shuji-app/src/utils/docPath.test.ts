import { describe, it, expect } from 'vitest';
import { docIdToPath } from './docPath';

describe('docIdToPath', () => {
  it('returns path as-is when already a .shuji path', () => {
    expect(docIdToPath('.shuji/designs/dsgn-001.md')).toBe('.shuji/designs/dsgn-001.md');
  });

  it('maps revw_ prefix to reviews', () => {
    expect(docIdToPath('revw_001')).toBe('.shuji/reviews/revw_001.md');
  });

  it('maps plan_ prefix to plans', () => {
    expect(docIdToPath('plan_001')).toBe('.shuji/plans/plan_001.md');
  });

  it('maps other ids to designs as default', () => {
    expect(docIdToPath('dsgn-001')).toBe('.shuji/designs/dsgn-001.md');
  });
});
