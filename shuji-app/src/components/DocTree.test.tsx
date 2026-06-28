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
});
