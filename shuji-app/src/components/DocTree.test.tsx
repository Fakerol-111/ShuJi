import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import DocTree from './DocTree';
import type { ShujiEntry } from '../api';

vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn().mockResolvedValue(() => {}),
}));

vi.mock('../api', () => ({
  listShujiTree: vi.fn(),
  getEditorConfig: vi.fn(),
  openInExternalEditor: vi.fn(),
  openProjectInExternalEditor: vi.fn(),
  onDocsMayHaveChanged: () => [Promise.resolve(() => {})],
}));

import * as api from '../api';

const mockedApi = vi.mocked(api);

const sampleTree: ShujiEntry[] = [
  {
    name: 'src',
    path: 'src',
    is_dir: true,
    type_label: 'Dir',
    children: [
      {
        name: 'main.rs',
        path: 'src/main.rs',
        is_dir: false,
        type_label: 'Rust',
        children: [],
      },
    ],
  },
  {
    name: '.shuji',
    path: '.shuji',
    is_dir: true,
    type_label: 'Dir',
    children: [
      {
        name: 'revw_001.md',
        path: '.shuji/reviews/revw_001.md',
        is_dir: false,
        type_label: 'Review',
        children: [],
      },
    ],
  },
];

describe('DocTree', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockedApi.listShujiTree.mockResolvedValue(sampleTree);
    mockedApi.getEditorConfig.mockResolvedValue({
      editor: 'vscode',
      custom_command: null,
      reuse_window: true,
    });
  });

  it('shows only .shuji entries by default', async () => {
    render(<DocTree projectDir="/test" selectedDoc={null} onSelect={vi.fn()} />);
    await waitFor(() => {
      expect(screen.getByText('.shuji')).toBeTruthy();
      expect(screen.getByText('revw_001.md')).toBeTruthy();
      expect(screen.queryByText('main.rs')).toBeFalsy();
    });
  });

  it('shows all files when toggled', async () => {
    const user = userEvent.setup();
    render(<DocTree projectDir="/test" selectedDoc={null} onSelect={vi.fn()} />);
    await waitFor(() => {
      expect(screen.getByText('revw_001.md')).toBeTruthy();
    });
    await user.click(screen.getByText('全部文件'));
    await waitFor(() => {
      expect(screen.getByText('main.rs')).toBeTruthy();
    });
  });

  it('shows localized type label for shuji docs', async () => {
    render(<DocTree projectDir="/test" selectedDoc={null} onSelect={vi.fn()} />);
    await waitFor(() => {
      expect(screen.getByText('审查')).toBeTruthy();
      expect(screen.queryByText('Review')).toBeFalsy();
    });
  });

  it('opens file in external editor from context menu', async () => {
    const user = userEvent.setup();
    mockedApi.openInExternalEditor.mockResolvedValue(undefined);
    render(<DocTree projectDir="/test" selectedDoc={null} onSelect={vi.fn()} />);
    await waitFor(() => expect(screen.getByText('revw_001.md')).toBeTruthy());
    await user.pointer({ keys: '[MouseRight>]', target: screen.getByText('revw_001.md') });
    await waitFor(() => expect(screen.getByText('用 VS Code 打开')).toBeTruthy());
    await user.click(screen.getByText('用 VS Code 打开'));
    await waitFor(() => {
      expect(mockedApi.openInExternalEditor).toHaveBeenCalledWith(
        '/test',
        '.shuji/reviews/revw_001.md'
      );
    });
  });

  it('opens project root in external editor', async () => {
    const user = userEvent.setup();
    mockedApi.openProjectInExternalEditor.mockResolvedValue(undefined);
    render(<DocTree projectDir="/test" selectedDoc={null} onSelect={vi.fn()} />);
    await waitFor(() => expect(screen.getByText('用 VS Code 打开项目')).toBeTruthy());
    await user.click(screen.getByText('用 VS Code 打开项目'));
    await waitFor(() => {
      expect(mockedApi.openProjectInExternalEditor).toHaveBeenCalledWith('/test');
    });
  });
});
