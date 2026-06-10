import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import DocPreview from './DocPreview';

// Mock the api module
vi.mock('../api', () => ({
  readShujiDoc: vi.fn(),
  setDocumentStatus: vi.fn(),
  sendMessage: vi.fn(),
  getDocumentDiff: vi.fn(),
  getDocumentLineage: vi.fn(),
}));

// Mock react-markdown + plugins
vi.mock('react-markdown', () => ({
  default: ({ children }: { children: string }) => <div data-testid="markdown">{children}</div>,
}));
vi.mock('remark-gfm', () => ({ default: () => {} }));
vi.mock('rehype-highlight', () => ({ default: () => {} }));

// Import the mocked module
import * as api from '../api';

const mockedApi = vi.mocked(api);

function shujiDoc(status: string = 'draft') {
  return {
    content: `---\nid: doc-001\ntype: plan\nauthor: 工部尚书\ntimestamp: 2026-01-01\nstatus: ${status}\n---\n\n## 实施计划\n\n第一阶段完成。`,
    path: '.shuji/doc-001.md',
  };
}

describe('DocPreview', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockedApi.readShujiDoc.mockResolvedValue(shujiDoc());
    mockedApi.getDocumentDiff.mockResolvedValue({
      diff: '+new code\n-old code',
      has_previous: true,
      added: 1,
      removed: 1,
    });
    mockedApi.getDocumentLineage.mockResolvedValue({
      id: 'doc-001',
      doc_type: 'plan',
      author: '工部尚书',
      timestamp: '2026-01-01T00:00:00Z',
      status: 'approved',
      refs: [],
      children: [],
    });
  });

  it('renders loading state initially', () => {
    // Keep the promise pending
    mockedApi.readShujiDoc.mockReturnValue(new Promise(() => {}));
    render(<DocPreview projectDir="/test" docPath=".shuji/doc-001.md" />);
    expect(screen.getByText('开卷中…')).toBeTruthy();
  });

  it('renders error state on API failure', async () => {
    mockedApi.readShujiDoc.mockRejectedValue(new Error('文件不存在'));
    render(<DocPreview projectDir="/test" docPath="nonexistent.md" />);
    await waitFor(() => {
      expect(screen.getByText(/文件不存在/)).toBeTruthy();
    });
  });

  it('renders document content after loading', async () => {
    render(<DocPreview projectDir="/test" docPath=".shuji/doc-001.md" />);
    await waitFor(() => {
      expect(screen.getByText(/实施计划/)).toBeTruthy();
    });
  });

  it('shows 待陛下朱批 banner for in_review documents', async () => {
    mockedApi.readShujiDoc.mockResolvedValue(shujiDoc('in_review'));
    render(<DocPreview projectDir="/test" docPath=".shuji/doc-001.md" />);
    await waitFor(() => {
      expect(screen.getByText('待陛下朱批')).toBeTruthy();
      expect(screen.getByText('准奏')).toBeTruthy();
      expect(screen.getByText('封还')).toBeTruthy();
    });
  });

  it('hides 朱批 banner for non-review documents', async () => {
    render(<DocPreview projectDir="/test" docPath=".shuji/doc-001.md" />);
    await waitFor(() => {
      expect(screen.queryByText('待陛下朱批')).toBeFalsy();
    });
  });

  it('calls setDocumentStatus with approved when 准奏 clicked', async () => {
    mockedApi.readShujiDoc.mockResolvedValue(shujiDoc('in_review'));
    mockedApi.setDocumentStatus.mockResolvedValue('ok');
    mockedApi.sendMessage.mockResolvedValue('ok');
    const user = userEvent.setup();

    render(<DocPreview projectDir="/test" docPath=".shuji/doc-001.md" />);
    await waitFor(() => {
      expect(screen.getByText('准奏')).toBeTruthy();
    });
    await user.click(screen.getByText('准奏'));
    expect(mockedApi.setDocumentStatus).toHaveBeenCalledWith('doc-001', 'approved', undefined);
  });

  it('calls setDocumentStatus with rejected when 封还 clicked', async () => {
    mockedApi.readShujiDoc.mockResolvedValue(shujiDoc('in_review'));
    mockedApi.setDocumentStatus.mockResolvedValue('ok');
    mockedApi.sendMessage.mockResolvedValue('ok');
    const user = userEvent.setup();

    render(<DocPreview projectDir="/test" docPath=".shuji/doc-001.md" />);
    await waitFor(() => {
      expect(screen.getByText('封还')).toBeTruthy();
    });
    await user.click(screen.getByText('封还'));
    expect(mockedApi.setDocumentStatus).toHaveBeenCalledWith('doc-001', 'rejected', undefined);
  });

  it('sends 御批 message after approval', async () => {
    mockedApi.readShujiDoc.mockResolvedValue(shujiDoc('in_review'));
    mockedApi.setDocumentStatus.mockResolvedValue('ok');
    mockedApi.sendMessage.mockResolvedValue('ok');
    const user = userEvent.setup();

    render(<DocPreview projectDir="/test" docPath=".shuji/doc-001.md" />);
    await waitFor(() => {
      expect(screen.getByText('准奏')).toBeTruthy();
    });
    // Type a comment
    await user.type(screen.getByPlaceholderText('御批备注（可选）...'), '甚好，准');
    await user.click(screen.getByText('准奏'));
    expect(mockedApi.setDocumentStatus).toHaveBeenCalledWith('doc-001', 'approved', '甚好，准');
    expect(mockedApi.sendMessage).toHaveBeenCalledWith(expect.stringContaining('甚好，准'));
  });

  it('renders diff tab when has_previous is true', async () => {
    render(<DocPreview projectDir="/test" docPath=".shuji/doc-001.md" />);
    await waitFor(() => {
      expect(screen.getByText('差异')).toBeTruthy();
      expect(screen.getByText('+1/-1')).toBeTruthy();
    });
  });

  it('renders lineage tab for .shuji markdown files', async () => {
    render(<DocPreview projectDir="/test" docPath=".shuji/doc-001.md" />);
    await waitFor(() => {
      expect(screen.getByText('血缘')).toBeTruthy();
    });
  });

  it('does not render lineage tab for non-shuji files', async () => {
    render(<DocPreview projectDir="/test" docPath="src/main.rs" />);
    await waitFor(() => {
      // Wait for loading to finish
      expect(screen.queryByText('血缘')).toBeFalsy();
    });
  });

  it('shows rejection reason dropdown options', async () => {
    mockedApi.readShujiDoc.mockResolvedValue(shujiDoc('in_review'));
    render(<DocPreview projectDir="/test" docPath=".shuji/doc-001.md" />);
    await waitFor(() => {
      expect(screen.getByText('驳回模板')).toBeTruthy();
    });
    const select = screen.getByText('驳回模板').closest('select') as HTMLSelectElement;
    expect(select).toBeTruthy();
  });
});
