import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';
import ArtifactPanel from './ArtifactPanel';

vi.mock('../api', () => ({
  readShujiDoc: vi.fn().mockResolvedValue({ content: '# doc', path: '.shuji/doc.md' }),
  setDocumentStatus: vi.fn().mockResolvedValue('ok'),
  sendMessage: vi.fn().mockResolvedValue('ok'),
  getDocumentDiff: vi
    .fn()
    .mockResolvedValue({ diff: '', has_previous: false, added: 0, removed: 0 }),
  getDocumentLineage: vi.fn().mockResolvedValue(null),
}));

vi.mock('react-markdown', () => ({
  default: ({ children }: { children: string }) => <div data-testid="markdown">{children}</div>,
}));
vi.mock('remark-gfm', () => ({ default: () => {} }));
vi.mock('rehype-highlight', () => ({ default: () => {} }));

const baseProps = {
  project: { working_dir: '/test' },
  tabs: [],
  activeIndex: 0,
  activeDoc: null,
  hasTabs: false,
  pendingApprovals: [],
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
    expect(screen.getByText('点击文档引用或阶段详情以预览')).toBeTruthy();
  });

  it('shows approval prompt when pending approvals exist', () => {
    render(<ArtifactPanel {...baseProps} pendingApprovals={['revw_001']} />);
    expect(screen.getByText('待陛下朱批')).toBeTruthy();
  });

  it('shows TabBar and DocPreview when tabs exist', async () => {
    const doc = { path: '.shuji/doc.md', label: 'doc' };
    render(<ArtifactPanel {...baseProps} hasTabs tabs={[doc]} activeDoc={doc} activeIndex={0} />);
    await waitFor(() => {
      expect(screen.getByText('开卷中…')).toBeTruthy();
    });
  });
});
