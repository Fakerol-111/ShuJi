import type { DeptLogEntry } from '../types';

const DOC_PATH_RE = /\.shuji\/[\w./-]+\.md/;
const ERROR_PREFIX = '❌';
const ROUTE_PREFIX = '→';

export type ActionClass = 'output' | 'error' | 'route' | 'action';

export function classifyDeptAction(entry: DeptLogEntry): ActionClass {
  if (entry.action.startsWith(ERROR_PREFIX)) return 'error';
  if (entry.action.startsWith(ROUTE_PREFIX)) return 'route';
  if (hasDocPath(entry)) return 'output';
  return 'action';
}

export function extractDocPath(entry: DeptLogEntry): string | null {
  const match = (entry.detail || entry.action).match(DOC_PATH_RE);
  return match ? match[0] : null;
}

function hasDocPath(entry: DeptLogEntry): boolean {
  const action = entry.action;
  if (action.includes('创建') || action.includes('修改') || action.includes('文档')) {
    return DOC_PATH_RE.test(action) || DOC_PATH_RE.test(entry.detail || '');
  }
  return false;
}

export function stripActionPrefix(action: string): string {
  return action.replace(/^[❌→]\s*/, '').replace(/:.*/, '');
}
