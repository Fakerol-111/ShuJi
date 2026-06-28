import { describe, it, expect, vi, beforeEach } from 'vitest';

const invokeMock = vi.fn();

vi.mock('@tauri-apps/api/core', () => ({
  invoke: (...args: unknown[]) => invokeMock(...args),
}));

describe('soul api', () => {
  beforeEach(() => {
    invokeMock.mockReset();
  });

  it('getSoulContent passes role and scope', async () => {
    invokeMock.mockResolvedValue('soul body');
    const { getSoulContent } = await import('./api');
    const content = await getSoulContent('Gongbushangshu', 'project');
    expect(invokeMock).toHaveBeenCalledWith('get_soul_content', {
      role: 'Gongbushangshu',
      scope: 'project',
    });
    expect(content).toBe('soul body');
  });

  it('clearSoul passes role and scope', async () => {
    invokeMock.mockResolvedValue(undefined);
    const { clearSoul } = await import('./api');
    await clearSoul('Neige', 'global');
    expect(invokeMock).toHaveBeenCalledWith('clear_soul', {
      role: 'Neige',
      scope: 'global',
    });
  });

  it('setLearningGlobalEnabled forwards enabled flag', async () => {
    invokeMock.mockResolvedValue(undefined);
    const { setLearningGlobalEnabled } = await import('./api');
    await setLearningGlobalEnabled(true);
    expect(invokeMock).toHaveBeenCalledWith('set_learning_global_enabled', {
      enabled: true,
    });
  });
});
