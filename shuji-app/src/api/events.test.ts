import { describe, it, expect, vi, beforeEach } from 'vitest';
import {
  isChatMessage,
  isChatDeltaEvent,
  isDeptLogEntry,
  isDeptStepEntry,
  isPlanInfo,
  isProject,
  isUsageUpdate,
  isRuntimeUpdate,
  TAURI_EVENTS,
  onChatMessage,
  onChatDelta,
  onChatComplete,
  onDeptLog,
  onDeptStep,
  onPlanUpdate,
  onProjectUpdate,
  onUsageUpdate,
  onRuntimeUpdate,
  onProjectChanged,
  onUsageChanged,
  onDocsMayHaveChanged,
} from './events';

vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn((_event: string, cb: (event: { payload: unknown }) => void) => {
    const g = globalThis as { __eventListeners?: Record<string, (payload: unknown) => void> };
    if (!g.__eventListeners) g.__eventListeners = {};
    g.__eventListeners[_event] = (payload: unknown) => cb({ payload });
    return Promise.resolve(() => {
      /* noop cleanup */
    });
  }),
}));

function getListeners(): Record<string, (payload: unknown) => void> {
  return (
    (globalThis as { __eventListeners?: Record<string, (payload: unknown) => void> })
      .__eventListeners ?? {}
  );
}

// Helper: build typed payloads without spreading `unknown`
function msg(overrides?: Partial<Record<string, unknown>>): Record<string, unknown> {
  return {
    id: 'abc',
    role: '皇帝',
    content: 'hi',
    options: [],
    documents: [],
    timestamp: 'now',
    ...overrides,
  };
}

function delta(overrides?: Partial<Record<string, unknown>>): Record<string, unknown> {
  return { message_id: 'm1', role: '内阁', delta: '...', ...overrides };
}

function log(overrides?: Partial<Record<string, unknown>>): Record<string, unknown> {
  return { dept: '工部', action: 'doing', ts: 'now', ...overrides };
}

function step(overrides?: Partial<Record<string, unknown>>): Record<string, unknown> {
  return { dept: '工部', ts: 'now', kind: { type: 'iteration', n: 1 }, ...overrides };
}

function plan(overrides?: Partial<Record<string, unknown>>): Record<string, unknown> {
  return {
    batches: [{ name: 'a', goal: 'do a', status: 'done' }],
    current: 1,
    complete: false,
    ...overrides,
  };
}

function project(overrides?: Partial<Record<string, unknown>>): Record<string, unknown> {
  return { id: 'p1', name: 'test', goal: 'build', working_dir: '/tmp', ...overrides };
}

function usage(overrides?: Partial<Record<string, unknown>>): Record<string, unknown> {
  return { role: 'Neige', kind: 'token', ...overrides };
}

function runtime(overrides?: Partial<Record<string, unknown>>): Record<string, unknown> {
  return { active_roles: ['Neige'], trigger: 'pipeline', ...overrides };
}

// ── Event name constants ──

describe('TAURI_EVENTS', () => {
  it('has all expected event names', () => {
    expect(TAURI_EVENTS.chatMessage).toBe('chat-message');
    expect(TAURI_EVENTS.chatDelta).toBe('chat-delta');
    expect(TAURI_EVENTS.chatComplete).toBe('chat-complete');
    expect(TAURI_EVENTS.deptLog).toBe('dept-log');
    expect(TAURI_EVENTS.deptStep).toBe('dept-step');
    expect(TAURI_EVENTS.planUpdate).toBe('plan-update');
    expect(TAURI_EVENTS.projectUpdate).toBe('project-update');
    expect(TAURI_EVENTS.usageUpdate).toBe('usage-update');
    expect(TAURI_EVENTS.runtimeUpdate).toBe('runtime-update');
  });
});

// ── Type guard: isChatMessage ──

describe('isChatMessage', () => {
  it('accepts valid', () => expect(isChatMessage(msg())).toBe(true));
  it('rejects null', () => expect(isChatMessage(null)).toBe(false));
  it('rejects missing id', () => expect(isChatMessage(msg({ id: undefined }))).toBe(false));
  it('rejects non-string role', () => expect(isChatMessage(msg({ role: 123 }))).toBe(false));
  it('accepts extra fields', () => expect(isChatMessage(msg({ extra: 'x' }))).toBe(true));
});

// ── Type guard: isChatDeltaEvent ──

describe('isChatDeltaEvent', () => {
  it('accepts valid', () => expect(isChatDeltaEvent(delta())).toBe(true));
  it('rejects missing delta', () =>
    expect(isChatDeltaEvent({ message_id: 'm1', role: '内阁' })).toBe(false));
});

// ── Type guard: isDeptLogEntry ──

describe('isDeptLogEntry', () => {
  it('accepts valid', () => expect(isDeptLogEntry(log())).toBe(true));
  it('accepts with detail', () => expect(isDeptLogEntry(log({ detail: 'x' }))).toBe(true));
  it('rejects missing action', () => expect(isDeptLogEntry({ dept: '工部', ts: 'x' })).toBe(false));
});

// ── Type guard: isDeptStepEntry ──

describe('isDeptStepEntry', () => {
  it('accepts valid', () => expect(isDeptStepEntry(step())).toBe(true));
  it('accepts tool_call kind', () =>
    expect(
      isDeptStepEntry(step({ kind: { type: 'tool_call', tool: 'read_file', args: {} } }))
    ).toBe(true));
  it('rejects missing kind', () => expect(isDeptStepEntry({ dept: '工部', ts: 'x' })).toBe(false));
  it('rejects kind without type', () =>
    expect(isDeptStepEntry(step({ kind: { n: 1 } }))).toBe(false));
});

// ── Type guard: isPlanInfo ──

describe('isPlanInfo', () => {
  it('accepts valid', () => expect(isPlanInfo(plan())).toBe(true));
  it('rejects missing batches', () =>
    expect(isPlanInfo({ current: 0, complete: false })).toBe(false));
  it('rejects non-boolean complete', () =>
    expect(isPlanInfo(plan({ complete: 'yes' }))).toBe(false));
});

// ── Type guard: isProject ──

describe('isProject', () => {
  it('accepts valid', () => expect(isProject(project())).toBe(true));
  it('rejects missing goal', () => expect(isProject({ id: 'p1', name: 'test' })).toBe(false));
});

// ── Type guard: isUsageUpdate ──

describe('isUsageUpdate', () => {
  it('accepts valid', () => expect(isUsageUpdate(usage())).toBe(true));
  it('accepts context kind', () => expect(isUsageUpdate(usage({ kind: 'context' }))).toBe(true));
  it('rejects unknown kind', () => expect(isUsageUpdate(usage({ kind: 'unknown' }))).toBe(false));
});

// ── Type guard: isRuntimeUpdate ──

describe('isRuntimeUpdate', () => {
  it('accepts valid', () => expect(isRuntimeUpdate(runtime())).toBe(true));
  it('accepts extra fields', () =>
    expect(isRuntimeUpdate(runtime({ round_metrics: null, pipeline: null }))).toBe(true));
  it('rejects non-array active_roles', () =>
    expect(isRuntimeUpdate(runtime({ active_roles: 'x' }))).toBe(false));
  it('rejects missing trigger', () => expect(isRuntimeUpdate({ active_roles: [] })).toBe(false));
});

// ── Invalid payloads dropped ──

describe('listeners drop invalid payloads', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('onChatMessage drops invalid', () => {
    const handler = vi.fn();
    onChatMessage(handler);
    getListeners()?.['chat-message']?.({ id: 123 });
    expect(handler).not.toHaveBeenCalled();
  });

  it('onPlanUpdate drops invalid', () => {
    const handler = vi.fn();
    onPlanUpdate(handler);
    getListeners()?.['plan-update']?.({ batches: 'bad', current: 0, complete: false });
    expect(handler).not.toHaveBeenCalled();
  });

  it('onUsageChanged drops invalid', () => {
    const handler = vi.fn();
    onUsageChanged(handler);
    getListeners()?.['usage-update']?.({ role: 42, kind: 'token' });
    expect(handler).not.toHaveBeenCalled();
  });
});

// ── Semantic wrappers ──

describe('semantic wrappers', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('onProjectChanged subscribes to project-update', () => {
    onProjectChanged(vi.fn());
    expect(getListeners()?.['project-update']).toBeDefined();
  });

  it('onDocsMayHaveChanged subscribes to 3 events', () => {
    expect(onDocsMayHaveChanged(() => {})).toHaveLength(3);
  });
});

// ── Subscription functions return UnlistenFn ──

describe('subscription functions return UnlistenFn', () => {
  it('all resolve to a function', async () => {
    await expect(onChatMessage(() => {})).resolves.toBeDefined();
    await expect(onChatDelta(() => {})).resolves.toBeDefined();
    await expect(onChatComplete(() => {})).resolves.toBeDefined();
    await expect(onDeptLog(() => {})).resolves.toBeDefined();
    await expect(onDeptStep(() => {})).resolves.toBeDefined();
    await expect(onPlanUpdate(() => {})).resolves.toBeDefined();
    await expect(onProjectUpdate(() => {})).resolves.toBeDefined();
    await expect(onUsageUpdate(() => {})).resolves.toBeDefined();
    await expect(onRuntimeUpdate(() => {})).resolves.toBeDefined();
  });
});
