import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import ArtifactPanel from './ArtifactPanel';
import { computeApprovalGateContext } from '../utils/approvalGate';

vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn().mockResolvedValue(() => {}),
}));

vi.mock('../api', () => ({
  readShujiDoc: vi.fn().mockResolvedValue({ content: '# doc', path: '.shuji/doc.md' }),
  setDocumentStatus: vi.fn().mockResolvedValue('ok'),
  sendMessage: vi.fn().mockResolvedValue('ok'),
  getDocumentDiff: vi
    .fn()
    .mockResolvedValue({ diff: '', has_previous: false, added: 0, removed: 0 }),
  getDocumentDiffs: vi.fn().mockResolvedValue([]),
  readDocumentDiff: vi.fn().mockResolvedValue(''),
  getDocumentLineage: vi.fn().mockResolvedValue(null),
  getEditorConfig: vi.fn().mockResolvedValue({
    editor: 'vscode',
    custom_command: null,
    reuse_window: true,
  }),
}));

vi.mock('react-markdown', () => ({
  default: ({ children }: { children: string }) => <div data-testid="markdown">{children}</div>,
}));
vi.mock('remark-gfm', () => ({ default: () => {} }));
vi.mock('rehype-highlight', () => ({ default: () => {} }));

const inactiveGate = computeApprovalGateContext([], null);

const baseProps = {
  project: { working_dir: '/test' },
  tabs: [],
  activeIndex: 0,
  activeDoc: null,
  hasTabs: false,
  pendingApprovals: [] as string[],
  gateContext: inactiveGate,
  onApproveDoc: vi.fn().mockResolvedValue(undefined),
  onSelectTab: vi.fn(),
  onCloseTab: vi.fn(),
  onClosePanel: vi.fn(),
  onOpenApproval: vi.fn(),
};

describe('ArtifactPanel', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('shows empty state when no tabs and no pending approvals', () => {
    render(<ArtifactPanel {...baseProps} />);
    expect(screen.getByText('请先开卷')).toBeTruthy();
  });

  it('shows 架阁 title instead of directory label', () => {
    render(<ArtifactPanel {...baseProps} />);
    expect(screen.getByText('架阁')).toBeTruthy();
  });

  it('closes panel via header button, not DocPreview', async () => {
    const onClosePanel = vi.fn();
    const doc = { path: '.shuji/doc.md', label: 'doc' };
    render(
      <ArtifactPanel
        {...baseProps}
        hasTabs
        tabs={[doc]}
        activeDoc={doc}
        activeIndex={0}
        onClosePanel={onClosePanel}
      />
    );
    await waitFor(() => {
      expect(screen.getByText('# doc')).toBeTruthy();
    });
    const closeButtons = screen.getAllByTitle('关闭');
    await userEvent.setup().click(closeButtons[0]);
    expect(onClosePanel).toHaveBeenCalledTimes(1);
  });

  it('shows approval prompt when pending approvals exist', () => {
    const gateContext = computeApprovalGateContext(['revw_001'], null);
    render(
      <ArtifactPanel {...baseProps} pendingApprovals={['revw_001']} gateContext={gateContext} />
    );
    expect(screen.getByText('待陛下朱批')).toBeTruthy();
    expect(screen.getByText('revw_001')).toBeTruthy();
  });

  it('shows TabBar and DocPreview when tabs exist', async () => {
    const doc = { path: '.shuji/doc.md', label: 'doc' };
    render(<ArtifactPanel {...baseProps} hasTabs tabs={[doc]} activeDoc={doc} activeIndex={0} />);
    await waitFor(() => {
      expect(screen.getByText('开卷中…')).toBeTruthy();
    });
  });

  it('closes tab via TabBar close button', async () => {
    const onCloseTab = vi.fn();
    const tabs = [
      { path: '.shuji/a.md', label: 'a' },
      { path: '.shuji/b.md', label: 'b' },
    ];
    render(
      <ArtifactPanel
        {...baseProps}
        hasTabs
        tabs={tabs}
        activeDoc={tabs[0]}
        activeIndex={0}
        onCloseTab={onCloseTab}
      />
    );
    const closeButtons = screen.getAllByLabelText(/关闭 a/);
    await userEvent.setup().click(closeButtons[0]);
    expect(onCloseTab).toHaveBeenCalledWith(0);
  });
});
