import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import DocPreview from './DocPreview';

vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn().mockResolvedValue(() => {}),
}));

vi.mock('../api', () => {
  const mockDone = () => Promise.resolve(() => {});
  return {
    readShujiDoc: vi.fn(),
    setDocumentStatus: vi.fn(),
    sendMessage: vi.fn(),
    getDocumentDiff: vi.fn(),
    getDocumentDiffs: vi.fn(),
    readDocumentDiff: vi.fn(),
    getDocumentLineage: vi.fn(),
    getEditorConfig: vi.fn(),
    openInExternalEditor: vi.fn(),
    onDocsMayHaveChanged: (handler: () => void) => {
      // Store handler in a shared global for test access
      const h = (globalThis as Record<string, unknown>).__docPreviewRefreshHandlers;
      if (Array.isArray(h)) h.push(handler);
      return [mockDone()];
    },
    onProjectUpdate: vi.fn().mockResolvedValue(mockDone()),
    onProjectChanged: vi.fn().mockResolvedValue(mockDone()),
  };
});

vi.mock('react-markdown', () => ({
  default: ({ children }: { children: string }) => <div data-testid="markdown">{children}</div>,
}));
vi.mock('remark-gfm', () => ({ default: () => {} }));
vi.mock('rehype-highlight', () => ({ default: () => {} }));

import * as api from '../api';

const mockedApi = vi.mocked(api);

function shujiDoc(status: string = 'draft') {
  return {
    content: `---\nid: doc-001\ntype: revw\nauthor: 门下侍中\ntimestamp: 2026-01-01\nstatus: ${status}\n---\n\n## 审查报告\n\n审查结论：建议准奏。`,
    path: '.shuji/reviews/doc-001.md',
  };
}

describe('DocPreview', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    (globalThis as Record<string, unknown>).__docPreviewRefreshHandlers = [];
    mockedApi.readShujiDoc.mockResolvedValue(shujiDoc());
    mockedApi.getDocumentDiffs.mockResolvedValue([
      { filename: 'doc-001_modify_2026.patch', event: 'modify', ts: '2026-01-02T00:00:00Z' },
    ]);
    mockedApi.readDocumentDiff.mockResolvedValue('+new line\n-old line');
    mockedApi.getDocumentDiff.mockResolvedValue({
      diff: '+new code\n-old code',
      has_previous: true,
      added: 1,
      removed: 1,
    });
    mockedApi.getDocumentLineage.mockResolvedValue({
      id: 'doc-001',
      doc_type: 'revw',
      author: '门下侍中',
      timestamp: '2026-01-01T00:00:00Z',
      status: 'approved',
      refs: [],
      children: [],
    });
    mockedApi.getEditorConfig.mockResolvedValue({
      editor: 'vscode',
      custom_command: null,
      reuse_window: true,
    });
  });

  it('renders loading state initially', () => {
    mockedApi.readShujiDoc.mockReturnValue(new Promise(() => {}));
    render(<DocPreview projectDir="/test" docPath=".shuji/reviews/doc-001.md" />);
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
    render(<DocPreview projectDir="/test" docPath=".shuji/reviews/doc-001.md" />);
    await waitFor(() => {
      expect(screen.getByText(/审查报告/)).toBeTruthy();
    });
  });

  it('shows 待陛下朱批 banner for in_review documents', async () => {
    mockedApi.readShujiDoc.mockResolvedValue(shujiDoc('in_review'));
    render(<DocPreview projectDir="/test" docPath=".shuji/reviews/doc-001.md" />);
    await waitFor(() => {
      expect(screen.getByText('待陛下朱批')).toBeTruthy();
      expect(screen.getByText('准奏')).toBeTruthy();
      expect(screen.queryByText('封还')).toBeFalsy();
    });
  });

  it('hides 朱批 banner for non-review documents', async () => {
    render(<DocPreview projectDir="/test" docPath=".shuji/reviews/doc-001.md" />);
    await waitFor(() => {
      expect(screen.queryByText('待陛下朱批')).toBeFalsy();
    });
  });

  it('calls setDocumentStatus with approved when 准奏 clicked', async () => {
    mockedApi.readShujiDoc.mockResolvedValue(shujiDoc('in_review'));
    mockedApi.setDocumentStatus.mockResolvedValue('ok');
    mockedApi.sendMessage.mockResolvedValue('ok');
    const user = userEvent.setup();

    render(<DocPreview projectDir="/test" docPath=".shuji/reviews/doc-001.md" />);
    await waitFor(() => {
      expect(screen.getByText('准奏')).toBeTruthy();
    });
    await user.click(screen.getByText('准奏'));
    expect(mockedApi.setDocumentStatus).toHaveBeenCalledWith('doc-001', 'approved', undefined);
  });

  it('sends 御批 message after approval', async () => {
    mockedApi.readShujiDoc.mockResolvedValue(shujiDoc('in_review'));
    mockedApi.setDocumentStatus.mockResolvedValue('ok');
    mockedApi.sendMessage.mockResolvedValue('ok');
    const user = userEvent.setup();

    render(<DocPreview projectDir="/test" docPath=".shuji/reviews/doc-001.md" />);
    await waitFor(() => {
      expect(screen.getByText('准奏')).toBeTruthy();
    });
    await user.type(screen.getByPlaceholderText('御批备注（可选）...'), '甚好，准');
    await user.click(screen.getByText('准奏'));
    expect(mockedApi.setDocumentStatus).toHaveBeenCalledWith('doc-001', 'approved', '甚好，准');
    expect(mockedApi.sendMessage).toHaveBeenCalledWith(expect.stringContaining('甚好，准'));
  });

  it('uses audit diff for .shuji markdown files', async () => {
    render(<DocPreview projectDir="/test" docPath=".shuji/reviews/doc-001.md" />);
    await waitFor(() => {
      expect(mockedApi.getDocumentDiffs).toHaveBeenCalledWith('doc-001');
      expect(mockedApi.readDocumentDiff).toHaveBeenCalled();
      expect(mockedApi.getDocumentDiff).not.toHaveBeenCalled();
    });
    await waitFor(() => {
      expect(screen.getByText('差异')).toBeTruthy();
      expect(screen.getByText('+1/-1')).toBeTruthy();
    });
  });

  it('uses git diff for non-shuji files', async () => {
    mockedApi.readShujiDoc.mockResolvedValue({ content: 'fn main() {}', path: 'src/main.rs' });
    render(<DocPreview projectDir="/test" docPath="src/main.rs" />);
    await waitFor(() => {
      expect(mockedApi.getDocumentDiff).toHaveBeenCalledWith('/test', 'src/main.rs');
      expect(mockedApi.getDocumentDiffs).not.toHaveBeenCalled();
    });
  });

  it('switches view when initialTab updates on same path', async () => {
    const { rerender } = render(
      <DocPreview projectDir="/test" docPath=".shuji/reviews/doc-001.md" initialTab="content" />
    );
    await waitFor(() => {
      expect(screen.getByText(/审查报告/)).toBeTruthy();
    });
    rerender(
      <DocPreview projectDir="/test" docPath=".shuji/reviews/doc-001.md" initialTab="diff" />
    );
    await waitFor(() => {
      expect(screen.getByText('+new line')).toBeTruthy();
    });
  });

  it('does not hide content during silent refresh', async () => {
    render(<DocPreview projectDir="/test" docPath=".shuji/reviews/doc-001.md" />);
    await waitFor(() => {
      expect(screen.getByText(/审查报告/)).toBeTruthy();
    });
    mockedApi.readShujiDoc.mockResolvedValue({
      ...shujiDoc(),
      content: shujiDoc().content.replace('审查报告', '更新后的审查报告'),
    });
    const refreshHandlers = (globalThis as Record<string, unknown>)
      .__docPreviewRefreshHandlers as Array<() => void>;
    const handler = refreshHandlers[refreshHandlers.length - 1];
    expect(handler).toBeTruthy();
    handler();
    await waitFor(() => {
      expect(screen.getByText(/更新后的审查报告/)).toBeTruthy();
      expect(screen.queryByText('开卷中…')).toBeFalsy();
    });
  });

  it('renders lineage tab for .shuji markdown files', async () => {
    render(<DocPreview projectDir="/test" docPath=".shuji/reviews/doc-001.md" />);
    await waitFor(() => {
      expect(screen.getByText('血缘')).toBeTruthy();
    });
  });

  it('does not render lineage tab for non-shuji files', async () => {
    mockedApi.readShujiDoc.mockResolvedValue({ content: 'fn main() {}', path: 'src/main.rs' });
    render(<DocPreview projectDir="/test" docPath="src/main.rs" />);
    await waitFor(() => {
      expect(screen.queryByText('血缘')).toBeFalsy();
    });
  });

  it('does not show rejection template dropdown', async () => {
    mockedApi.readShujiDoc.mockResolvedValue(shujiDoc('in_review'));
    render(<DocPreview projectDir="/test" docPath=".shuji/reviews/doc-001.md" />);
    await waitFor(() => {
      expect(screen.getByText('准奏')).toBeTruthy();
    });
    expect(screen.queryByText('驳回模板')).toBeFalsy();
  });

  it('uses IDE-style shell with overflow scroll containers', async () => {
    const { container } = render(
      <DocPreview projectDir="/test" docPath=".shuji/reviews/doc-001.md" />
    );
    await waitFor(() => {
      expect(screen.getByText(/审查报告/)).toBeTruthy();
    });
    expect(container.querySelector('.doc-preview-shell')).toBeTruthy();
    expect(container.querySelector('.doc-preview-body')).toBeTruthy();
    expect(container.querySelector('.doc-preview-markdown')).toBeTruthy();
    const body = container.querySelector('.doc-preview-body');
    expect(body?.className).toMatch(/overflow-auto/);
  });

  it('renders collapsible metadata instead of frontmatter card', async () => {
    const { container } = render(
      <DocPreview projectDir="/test" docPath=".shuji/reviews/doc-001.md" />
    );
    await waitFor(() => {
      expect(screen.getByText(/审查报告/)).toBeTruthy();
    });
    expect(container.querySelector('.doc-preview-metadata')).toBeTruthy();
    expect(container.querySelector('details')).toBeTruthy();
  });

  it('opens file in external editor from toolbar button', async () => {
    const user = userEvent.setup();
    mockedApi.openInExternalEditor.mockResolvedValue(undefined);
    render(<DocPreview projectDir="/test" docPath=".shuji/reviews/doc-001.md" />);
    await waitFor(() => {
      expect(screen.getByText(/审查报告/)).toBeTruthy();
    });
    await user.click(screen.getByRole('button', { name: '用 VS Code 打开' }));
    await waitFor(() => {
      expect(mockedApi.openInExternalEditor).toHaveBeenCalledWith(
        '/test',
        '.shuji/reviews/doc-001.md'
      );
    });
  });

  it('shows editor error below toolbar on open failure', async () => {
    const user = userEvent.setup();
    mockedApi.openInExternalEditor.mockRejectedValue(new Error('未找到 code'));
    render(<DocPreview projectDir="/test" docPath=".shuji/reviews/doc-001.md" />);
    await waitFor(() => {
      expect(screen.getByText(/审查报告/)).toBeTruthy();
    });
    await user.click(screen.getByRole('button', { name: '用 VS Code 打开' }));
    await waitFor(() => {
      expect(screen.getByText(/未找到 code/)).toBeTruthy();
    });
  });

  it('opens code file at clicked line number', async () => {
    const user = userEvent.setup();
    mockedApi.openInExternalEditor.mockResolvedValue(undefined);
    mockedApi.readShujiDoc.mockResolvedValue({
      content: 'line1\nline2\nline3',
      path: 'src/main.rs',
    });
    render(<DocPreview projectDir="/test" docPath="src/main.rs" />);
    await waitFor(() => expect(screen.getByText('2')).toBeTruthy());
    await user.click(screen.getByText('2'));
    await waitFor(() => {
      expect(mockedApi.openInExternalEditor).toHaveBeenCalledWith('/test', 'src/main.rs', 2);
    });
  });
});
