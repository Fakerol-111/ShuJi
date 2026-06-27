import { describe, it, expect } from 'vitest';
import { getDeptDisplayLabel } from '../constants';
import type { DeptMeta } from '../constants';

const meta: DeptMeta = {
  key: 'zhongshuling',
  label: '中书令',
  shortLabel: '中书',
  description: '方案设计',
  labelEn: 'Architect',
  shortLabelEn: 'Architect',
  descriptionEn: 'Design',
  color: '#000',
  bg: '',
  text: '',
  accent: '',
};

describe('getDeptDisplayLabel', () => {
  it('returns Chinese short label in zh mode', () => {
    expect(getDeptDisplayLabel(meta, 'zh')).toBe('中书');
  });

  it('returns bilingual label in en mode', () => {
    expect(getDeptDisplayLabel(meta, 'en')).toBe('Architect · 中书');
  });
});

describe('uiPrefs', () => {
  it('defaults experience level to beginner', async () => {
    localStorage.removeItem('shuji_ui_prefs');
    const { getExperienceLevel } = await import('./uiPrefs');
    expect(getExperienceLevel()).toBe('beginner');
  });
});
