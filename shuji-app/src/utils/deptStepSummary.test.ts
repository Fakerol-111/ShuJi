import { describe, it, expect } from 'vitest';
import {
  summarizeDeptStep,
  deriveRecentHumanActions,
  deriveLatestHumanSummary,
  deriveLatestStepByDept,
} from './deptStepSummary';
import type { DeptStepEntry } from '../types';

function step(
  dept: string,
  kind: DeptStepEntry['kind'],
  ts = '2026-06-27T10:00:00+08:00'
): DeptStepEntry {
  return { dept, ts, kind };
}

describe('summarizeDeptStep', () => {
  it('maps read_file to readable Chinese action', () => {
    const entry = step('工部尚书', {
      type: 'tool_call',
      tool: 'read_file',
      args: { path: 'src/main.rs' },
    });
    expect(summarizeDeptStep(entry)).toBe('工部尚书正在读取文件');
  });

  it('includes doc id when present in args', () => {
    const entry = step('中书令', {
      type: 'tool_call',
      tool: 'append_document',
      args: { doc_id: 'dsgn_001' },
    });
    expect(summarizeDeptStep(entry)).toContain('dsgn_001');
  });

  it('maps run_tests for 刑部尚书', () => {
    const entry = step('刑部尚书', { type: 'tool_call', tool: 'run_tests', args: {} });
    expect(summarizeDeptStep(entry)).toBe('刑部尚书正在运行测试');
  });

  it('summarizes text_delta as responding', () => {
    const entry = step('工部尚书', { type: 'text_delta', delta: 'hello' });
    expect(summarizeDeptStep(entry)).toBe('工部尚书正在输出…');
  });

  it('returns null for iteration events', () => {
    const entry = step('内阁', { type: 'iteration', n: 3 });
    expect(summarizeDeptStep(entry)).toBeNull();
  });

  it('summarizes failed tool results', () => {
    const entry = step('工部尚书', {
      type: 'tool_result',
      tool: 'run_tests',
      ok: false,
      summary: '3 failed',
    });
    expect(summarizeDeptStep(entry)).toContain('失败');
  });
});

describe('deriveRecentHumanActions', () => {
  it('returns newest actions across departments', () => {
    const map = new Map<string, DeptStepEntry[]>([
      [
        '工部尚书',
        [
          step(
            '工部尚书',
            { type: 'tool_call', tool: 'read_file', args: {} },
            '2026-06-27T10:00:01+08:00'
          ),
        ],
      ],
      [
        '兵部尚书',
        [
          step(
            '兵部尚书',
            { type: 'tool_call', tool: 'append_document', args: { doc_id: 'ctrt_001' } },
            '2026-06-27T10:00:02+08:00'
          ),
        ],
      ],
    ]);
    const actions = deriveRecentHumanActions(map, 'zh', 2);
    expect(actions).toHaveLength(2);
    expect(actions[1].dept).toBe('兵部尚书');
  });
});

describe('deriveLatestHumanSummary', () => {
  it('prefers active department action', () => {
    const map = new Map<string, DeptStepEntry[]>([
      [
        '中书令',
        [
          step(
            '中书令',
            { type: 'tool_call', tool: 'read_document', args: {} },
            '2026-06-27T10:00:01+08:00'
          ),
        ],
      ],
      [
        '工部尚书',
        [
          step(
            '工部尚书',
            { type: 'tool_call', tool: 'edit_file', args: {} },
            '2026-06-27T10:00:02+08:00'
          ),
        ],
      ],
    ]);
    const latest = deriveLatestHumanSummary(map, ['工部尚书'], 'zh');
    expect(latest?.dept).toBe('工部尚书');
  });
});

describe('deriveLatestStepByDept', () => {
  it('skips iteration-only steps', () => {
    const map = new Map<string, DeptStepEntry[]>([
      [
        '内阁',
        [
          step('内阁', { type: 'iteration', n: 1 }, '2026-06-27T10:00:00+08:00'),
          step(
            '内阁',
            { type: 'tool_call', tool: 'submit_pipeline_plan', args: {} },
            '2026-06-27T10:00:01+08:00'
          ),
        ],
      ],
    ]);
    const latest = deriveLatestStepByDept(map);
    expect(latest.get('内阁')?.kind.type).toBe('tool_call');
  });
});

describe('waiting heartbeat', () => {
  it('summarizes waiting event with elapsed seconds', () => {
    const entry = step('内阁', { type: 'waiting', elapsed_secs: 6 });
    const result = summarizeDeptStep(entry, 'zh');
    expect(result).toContain('6s');
    expect(result).toContain('思考');
  });

  it('summarizes waiting event in English', () => {
    const entry = step('内阁', { type: 'waiting', elapsed_secs: 9 });
    const result = summarizeDeptStep(entry, 'en');
    expect(result).toContain('9s');
    expect(result).toContain('thinking');
  });
});
