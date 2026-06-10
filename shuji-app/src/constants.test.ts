import { describe, it, expect } from 'vitest';
import {
  DEPT_META_LIST,
  DEPT_META,
  DEPT_META_BY_KEY,
  DEPT_ORDER,
  getDeptMeta,
  ALL_ROLES,
  ROLE_CONTEXT_DEFAULTS,
} from './constants';

describe('DEPT_META_LIST', () => {
  it('has 9 departments', () => {
    expect(DEPT_META_LIST).toHaveLength(9);
  });

  it('includes all required departments', () => {
    const labels = DEPT_META_LIST.map((d) => d.label);
    expect(labels).toContain('内阁');
    expect(labels).toContain('中书令');
    expect(labels).toContain('门下侍中');
    expect(labels).toContain('尚书令');
    expect(labels).toContain('吏部尚书');
    expect(labels).toContain('兵部尚书');
    expect(labels).toContain('工部尚书');
    expect(labels).toContain('刑部尚书');
    expect(labels).toContain('礼部尚书');
  });

  it('each entry has required fields', () => {
    for (const dept of DEPT_META_LIST) {
      expect(dept.key).toBeTruthy();
      expect(dept.label).toBeTruthy();
      expect(dept.shortLabel).toBeTruthy();
      expect(dept.description).toBeTruthy();
      expect(dept.color).toMatch(/^#[0-9A-Fa-f]{6}$/);
      expect(dept.bg).toMatch(/^bg-/);
      expect(dept.text).toMatch(/^text-/);
      expect(dept.accent).toMatch(/^border-l-/);
    }
  });
});

describe('DEPT_META', () => {
  it('indexes by Chinese label', () => {
    expect(DEPT_META['内阁']).toBeDefined();
    expect(DEPT_META['内阁'].key).toBe('neige');
    expect(DEPT_META['工部尚书']).toBeDefined();
    expect(DEPT_META['工部尚书'].key).toBe('gongbushangshu');
  });
});

describe('DEPT_META_BY_KEY', () => {
  it('indexes by English key', () => {
    expect(DEPT_META_BY_KEY['neige']).toBeDefined();
    expect(DEPT_META_BY_KEY['neige'].label).toBe('内阁');
    expect(DEPT_META_BY_KEY['gongbushangshu']).toBeDefined();
    expect(DEPT_META_BY_KEY['gongbushangshu'].label).toBe('工部尚书');
  });
});

describe('DEPT_ORDER', () => {
  it('matches order of DEPT_META_LIST', () => {
    expect(DEPT_ORDER).toEqual(DEPT_META_LIST.map((d) => d.label));
    expect(DEPT_ORDER).toHaveLength(9);
  });
});

describe('getDeptMeta', () => {
  it('resolves by Chinese label', () => {
    expect(getDeptMeta('内阁')?.key).toBe('neige');
    expect(getDeptMeta('工部尚书')?.key).toBe('gongbushangshu');
  });

  it('resolves by short label', () => {
    expect(getDeptMeta('工部')?.key).toBe('gongbushangshu');
    expect(getDeptMeta('吏部')?.key).toBe('libushangshu');
    expect(getDeptMeta('兵部')?.key).toBe('bingbushangshu');
  });

  it('resolves by full English key', () => {
    expect(getDeptMeta('zhongshuling')?.label).toBe('中书令');
    expect(getDeptMeta('menxiashizhong')?.label).toBe('门下侍中');
  });

  it('resolves by abbreviated English alias', () => {
    expect(getDeptMeta('zhongshu')?.label).toBe('中书令');
    expect(getDeptMeta('menxia')?.label).toBe('门下侍中');
  });

  it('returns undefined for unknown keys', () => {
    expect(getDeptMeta('nonexistent')).toBeUndefined();
    expect(getDeptMeta('')).toBeUndefined();
  });
});

describe('ALL_ROLES', () => {
  it('includes default + 9 departments', () => {
    expect(ALL_ROLES).toHaveLength(10);
  });

  it('default role comes first', () => {
    expect(ALL_ROLES[0].key).toBe('default');
    expect(ALL_ROLES[0].label).toBe('默认（全局）');
  });
});

describe('ROLE_CONTEXT_DEFAULTS', () => {
  it('has entries for each department (by short label or full label)', () => {
    for (const dept of DEPT_META_LIST) {
      // ROLE_CONTEXT_DEFAULTS uses full labels for some (中书令), short for others (工部)
      const found = ROLE_CONTEXT_DEFAULTS[dept.label] || ROLE_CONTEXT_DEFAULTS[dept.shortLabel];
      expect(found).toBeDefined();
    }
  });

  it('all entries have 750000 token threshold', () => {
    for (const config of Object.values(ROLE_CONTEXT_DEFAULTS)) {
      expect(config.token_threshold).toBe(750_000);
    }
  });
});
