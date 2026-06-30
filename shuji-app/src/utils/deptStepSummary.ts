import { getDeptMeta } from '../constants';
import { basenameFromPath } from './pathBasename';
import type { DeptStepEntry, DeptStepKind } from '../types';

export type Lang = 'zh' | 'en';

export interface HumanAction {
  dept: string;
  summary: string;
  ts: string;
}

export interface DeptActivitySummary {
  status: 'idle' | 'active' | 'error' | 'waiting_approval';
  intent: string;
  latestAction: string;
  latestArtifact: string | null;
}

const TOOL_ACTIONS: Record<string, { zh: string; en: string }> = {
  read_file: { zh: '正在读取文件', en: 'Reading file' },
  read_document: { zh: '正在阅读文档', en: 'Reading document' },
  list_dir: { zh: '正在浏览目录', en: 'Browsing directory' },
  list_dir_tree: { zh: '正在扫描项目结构', en: 'Scanning project structure' },
  search_text: { zh: '正在搜索代码', en: 'Searching code' },
  create: { zh: '正在创建文件', en: 'Creating file' },
  edit_file: { zh: '正在修改文件', en: 'Editing file' },
  apply_patch: { zh: '正在应用补丁', en: 'Applying patch' },
  delete: { zh: '正在删除文件', en: 'Deleting file' },
  rename: { zh: '正在重命名文件', en: 'Renaming file' },
  modify_file: { zh: '正在修改文件', en: 'Modifying file' },
  append_file: { zh: '正在追加文件内容', en: 'Appending to file' },
  create_document: { zh: '正在创建文档', en: 'Creating document' },
  modify_document: { zh: '正在修改文档', en: 'Modifying document' },
  append_document: { zh: '正在撰写文档', en: 'Writing document' },
  set_document_status: { zh: '正在更新文档状态', en: 'Updating document status' },
  run_tests: { zh: '正在运行测试', en: 'Running tests' },
  execute_command: { zh: '正在执行命令', en: 'Executing command' },
  route_to: { zh: '正在转交任务', en: 'Handing off task' },
  submit_pipeline_plan: { zh: '正在提交执行计划', en: 'Submitting pipeline plan' },
  submit_plan: { zh: '正在制定批次计划', en: 'Planning batches' },
  complete_task: { zh: '正在完成任务', en: 'Completing task' },
  cancel_agent: { zh: '正在叫停部门', en: 'Stopping department' },
  update_soul: { zh: '正在更新经验库', en: 'Updating soul' },
  create_skill: { zh: '正在创建技能', en: 'Creating skill' },
  init_checklist: { zh: '正在初始化审计清单', en: 'Initializing audit checklist' },
  update_checklist: { zh: '正在更新审计清单', en: 'Updating audit checklist' },
  add_violation: { zh: '正在记录违规项', en: 'Recording violation' },
  request_reauth: { zh: '正在请求复核', en: 'Requesting re-auth' },
};

const DEPT_INTENT: Record<string, { zh: string; en: string }> = {
  neige: { zh: '编排', en: 'Orchestrating' },
  zhongshuling: { zh: '设计', en: 'Designing' },
  menxiashizhong: { zh: '审查', en: 'Reviewing' },
  shangshuling: { zh: '调度', en: 'Dispatching' },
  libushangshu: { zh: '拆解', en: 'Breaking down' },
  bingbushangshu: { zh: '测试设计', en: 'Test design' },
  gongbushangshu: { zh: '编码', en: 'Coding' },
  xingbushangshu: { zh: '验证', en: 'Verifying' },
  liburshangshu: { zh: '审计', en: 'Auditing' },
};

const DOC_ID_RE = /\b([a-z]{3,5}_\d{3,})\b/i;
const SHUJI_DOC_RE = /\.shuji\/[\w./-]+\.md/;

function pickLang(lang: Lang): 'zh' | 'en' {
  return lang === 'en' ? 'en' : 'zh';
}

function extractArtifact(
  args: Record<string, unknown> | undefined,
  summary?: string
): string | null {
  if (args) {
    for (const key of ['doc_id', 'id', 'document_id', 'path', 'file_path', 'subject']) {
      const val = args[key];
      if (typeof val === 'string') {
        const docMatch = val.match(DOC_ID_RE);
        if (docMatch) return docMatch[1];
        const pathMatch = val.match(SHUJI_DOC_RE);
        if (pathMatch) {
          const base = basenameFromPath(pathMatch[0]);
          return base.replace('.md', '');
        }
        if (val.includes('.shuji/')) {
          const base = basenameFromPath(val);
          return base.replace('.md', '');
        }
      }
    }
  }
  if (summary) {
    const docMatch = summary.match(DOC_ID_RE);
    if (docMatch) return docMatch[1];
  }
  return null;
}

function toolActionLabel(tool: string, lang: Lang): string {
  const l = pickLang(lang);
  return TOOL_ACTIONS[tool]?.[l] ?? (l === 'zh' ? `正在调用 ${tool}` : `Calling ${tool}`);
}

function deptIntent(dept: string, lang: Lang): string {
  const l = pickLang(lang);
  const meta = getDeptMeta(dept);
  if (meta && DEPT_INTENT[meta.key]) return DEPT_INTENT[meta.key][l];
  return l === 'zh' ? '处理中' : 'Working';
}

function summarizeToolCall(
  dept: string,
  tool: string,
  args: Record<string, unknown> | undefined,
  lang: Lang
): string {
  const l = pickLang(lang);
  const action = toolActionLabel(tool, lang);
  const artifact = extractArtifact(args);
  const deptLabel = getDeptMeta(dept)?.label ?? dept;

  if (tool === 'set_document_status' && args?.status === 'approved') {
    return l === 'zh' ? `${deptLabel}已准奏文档` : `${deptLabel} approved document`;
  }
  if (artifact) {
    return l === 'zh'
      ? `${deptLabel}${action.replace('正在', '')}（${artifact}）`
      : `${deptLabel}: ${action} (${artifact})`;
  }
  return l === 'zh' ? `${deptLabel}${action}` : `${deptLabel}: ${action}`;
}

function summarizeToolResult(
  dept: string,
  tool: string,
  ok: boolean,
  summary: string,
  lang: Lang
): string | null {
  const l = pickLang(lang);
  const deptLabel = getDeptMeta(dept)?.label ?? dept;
  const artifact = extractArtifact(undefined, summary);

  if (!ok) {
    return l === 'zh'
      ? `${deptLabel}执行失败：${toolActionLabel(tool, lang).replace('正在', '')}`
      : `${deptLabel} failed: ${tool}`;
  }

  if (tool.includes('document') || tool === 'create' || tool === 'append_document') {
    const msg =
      l === 'zh'
        ? `${deptLabel}已产出${artifact ? ` ${artifact}` : '文档'}`
        : `${deptLabel} produced${artifact ? ` ${artifact}` : ' document'}`;
    if (tool === 'set_document_status') {
      return l === 'zh'
        ? `${deptLabel}已更新文档状态${artifact ? `（${artifact}）` : ''}`
        : `${deptLabel} updated document status${artifact ? ` (${artifact})` : ''}`;
    }
    return msg;
  }

  if (tool === 'run_tests') {
    return l === 'zh' ? `${deptLabel}测试执行完毕` : `${deptLabel} finished running tests`;
  }

  return null;
}

/** Convert a single dept-step event to a human-readable line, or null if not worth showing. */
export function summarizeDeptStep(entry: DeptStepEntry, lang: Lang = 'zh'): string | null {
  const kind = entry.kind;
  return summarizeDeptStepKind(entry.dept, kind, lang);
}

export function summarizeDeptStepKind(
  dept: string,
  kind: DeptStepKind,
  lang: Lang = 'zh'
): string | null {
  const l = pickLang(lang);
  const deptLabel = getDeptMeta(dept)?.label ?? dept;

  switch (kind.type) {
    case 'tool_call':
      return summarizeToolCall(dept, kind.tool, kind.args, lang);
    case 'text_delta':
      return l === 'zh' ? `${deptLabel}正在输出…` : `${deptLabel} is responding…`;
    case 'reasoning_delta':
      return l === 'zh' ? `${deptLabel}正在思考…` : `${deptLabel} is thinking…`;
    case 'tool_result': {
      const result = summarizeToolResult(dept, kind.tool, kind.ok, kind.summary, lang);
      if (result) return result;
      if (!kind.ok) {
        return l === 'zh' ? `${deptLabel}工具执行失败` : `${deptLabel} tool execution failed`;
      }
      return null;
    }
    case 'text': {
      const preview = kind.content.trim().slice(0, 60);
      if (!preview) return null;
      return l === 'zh' ? `${deptLabel}：${preview}` : `${deptLabel}: ${preview}`;
    }
    case 'thinking':
      return l === 'zh' ? `${deptLabel}正在思考…` : `${deptLabel} is thinking…`;
    default:
      return null;
  }
}

/** Latest meaningful step per department. */
export function deriveLatestStepByDept(
  deptSteps: Map<string, DeptStepEntry[]>
): Map<string, DeptStepEntry> {
  const result = new Map<string, DeptStepEntry>();
  for (const [dept, steps] of deptSteps) {
    for (let i = steps.length - 1; i >= 0; i--) {
      const step = steps[i];
      if (summarizeDeptStep(step)) {
        result.set(dept, step);
        break;
      }
    }
  }
  return result;
}

/** Recent human-readable actions across all departments (newest last). */
export function deriveRecentHumanActions(
  deptSteps: Map<string, DeptStepEntry[]>,
  lang: Lang = 'zh',
  maxCount = 4
): HumanAction[] {
  const all: HumanAction[] = [];
  for (const [dept, steps] of deptSteps) {
    for (const step of steps) {
      const summary = summarizeDeptStep(step, lang);
      if (summary) {
        all.push({ dept, summary, ts: step.ts });
      }
    }
  }
  all.sort((a, b) => a.ts.localeCompare(b.ts));
  return all.slice(-maxCount);
}

/** The single most recent action, preferring active departments. */
export function deriveLatestHumanSummary(
  deptSteps: Map<string, DeptStepEntry[]>,
  activeDepts: string[],
  lang: Lang = 'zh'
): HumanAction | null {
  const recent = deriveRecentHumanActions(deptSteps, lang, 8);
  if (recent.length === 0) return null;

  for (let i = recent.length - 1; i >= 0; i--) {
    const action = recent[i];
    if (activeDepts.some((d) => d === action.dept || getDeptMeta(d)?.label === action.dept)) {
      return action;
    }
  }
  return recent[recent.length - 1];
}

/** Per-department activity summary for card rail. */
export function deriveDeptActivitySummary(
  dept: string,
  steps: DeptStepEntry[],
  active: boolean,
  hasError: boolean,
  lang: Lang = 'zh'
): DeptActivitySummary {
  let latestAction = '';
  let latestArtifact: string | null = null;

  for (let i = steps.length - 1; i >= 0; i--) {
    const step = steps[i];
    const summary = summarizeDeptStep(step, lang);
    if (summary) {
      latestAction = summary;
      const stepKind = step.kind;
      if (stepKind.type === 'tool_call') {
        latestArtifact = extractArtifact(stepKind.args);
      } else if (stepKind.type === 'tool_result') {
        latestArtifact = extractArtifact(undefined, stepKind.summary);
      }
      break;
    }
  }

  let status: DeptActivitySummary['status'] = 'idle';
  if (hasError) status = 'error';
  else if (active) status = 'active';
  else if (
    latestAction.includes('等待朱批') ||
    latestAction.includes('waiting approval') ||
    (latestArtifact && latestAction.includes('revw'))
  ) {
    status = 'waiting_approval';
  }

  return {
    status,
    intent: deptIntent(dept, lang),
    latestAction,
    latestArtifact,
  };
}

/** Waiting-for-approval message when review doc is produced. */
export function waitingApprovalSummary(dept: string, docId: string, lang: Lang = 'zh'): string {
  const l = pickLang(lang);
  const deptLabel = getDeptMeta(dept)?.label ?? dept;
  return l === 'zh'
    ? `${deptLabel}已产出审查报告 ${docId}，等待朱批`
    : `${deptLabel} produced review ${docId}, awaiting approval`;
}
