import { describe, it, expect } from 'vitest';
import { extractDocPath, classifyDeptAction, stripActionPrefix, isDeptActive } from './deptLog';
import type { DeptLogEntry } from '../types';

function entry(overrides: Partial<DeptLogEntry> = {}): DeptLogEntry {
  return {
    dept: '工部',
    action: '创建文档 dsgn-001',
    ts: '14:32:05',
    detail: '',
    ...overrides,
  };
}

describe('classifyDeptAction', () => {
  it('returns error for actions starting with ❌', () => {
    expect(classifyDeptAction(entry({ action: '❌ 测试失败' }))).toBe('error');
  });

  it('returns route for actions starting with →', () => {
    expect(classifyDeptAction(entry({ action: '→ 转交吏部' }))).toBe('route');
  });

  it('returns output for create/modify actions with doc path', () => {
    expect(classifyDeptAction(entry({ action: '创建文档 .shuji/designs/dsgn-001.md' }))).toBe(
      'output'
    );
  });

  it('returns action for plain actions', () => {
    expect(classifyDeptAction(entry({ action: '开始处理' }))).toBe('action');
  });
});

describe('extractDocPath', () => {
  it('extracts .shuji path from action', () => {
    const e = entry({ action: '修改文档 .shuji/designs/dsgn-001.md' });
    expect(extractDocPath(e)).toBe('.shuji/designs/dsgn-001.md');
  });

  it('extracts .shuji path from detail', () => {
    const e = entry({ action: '创建文档', detail: '路径: .shuji/plans/plan-001.md' });
    expect(extractDocPath(e)).toBe('.shuji/plans/plan-001.md');
  });

  it('returns null when no .shuji path', () => {
    const e = entry({ action: '开始执行' });
    expect(extractDocPath(e)).toBeNull();
  });
});

describe('stripActionPrefix', () => {
  it('strips ❌ prefix', () => {
    expect(stripActionPrefix('❌ 测试失败')).toBe('测试失败');
  });

  it('strips → prefix', () => {
    expect(stripActionPrefix('→ 转交吏部')).toBe('转交吏部');
  });

  it('strips suffix after colon', () => {
    expect(stripActionPrefix('创建文档: dsgn-001')).toBe('创建文档');
  });

  it('returns original string when no prefix', () => {
    expect(stripActionPrefix('开始执行')).toBe('开始执行');
  });
});

describe('isDeptActive', () => {
  it('returns true when label matches activeDepts', () => {
    expect(isDeptActive('工部尚书', ['工部尚书', '内阁'])).toBe(true);
  });

  it('returns true when shortLabel matches activeDepts', () => {
    expect(isDeptActive('工部尚书', ['工部', '内阁'])).toBe(true);
  });

  it('returns true when key matches activeDepts', () => {
    expect(isDeptActive('工部尚书', ['gongbushangshu'])).toBe(true);
  });

  it('returns false when dept is not in activeDepts', () => {
    expect(isDeptActive('工部尚书', ['内阁'])).toBe(false);
  });

  it('returns false when activeDepts is empty', () => {
    expect(isDeptActive('工部尚书', [])).toBe(false);
  });
});
