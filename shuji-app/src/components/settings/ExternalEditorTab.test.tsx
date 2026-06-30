import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import ExternalEditorTab from './ExternalEditorTab';

vi.mock('../../api', () => ({
  checkExternalEditor: vi.fn(),
  getEditorConfig: vi.fn(),
  setEditorConfig: vi.fn(),
}));

import * as api from '../../api';

const mockedApi = vi.mocked(api);

describe('ExternalEditorTab', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockedApi.getEditorConfig.mockResolvedValue({
      editor: 'vscode',
      custom_command: null,
      reuse_window: true,
    });
    mockedApi.checkExternalEditor.mockResolvedValue('ok');
    mockedApi.setEditorConfig.mockResolvedValue(undefined);
  });

  it('loads and displays editor options', async () => {
    render(<ExternalEditorTab setSavedMsg={vi.fn()} />);
    await waitFor(() => {
      expect(screen.getByText('VS Code')).toBeTruthy();
      expect(screen.getByText('Cursor')).toBeTruthy();
    });
  });

  it('shows custom command field when custom is selected', async () => {
    const user = userEvent.setup();
    render(<ExternalEditorTab setSavedMsg={vi.fn()} />);
    await waitFor(() => expect(screen.getByText('自定义')).toBeTruthy());
    await user.click(screen.getByText('自定义'));
    expect(screen.getByPlaceholderText(/MyEditor/)).toBeTruthy();
  });

  it('saves config on save click', async () => {
    const user = userEvent.setup();
    const setSavedMsg = vi.fn();
    render(<ExternalEditorTab setSavedMsg={setSavedMsg} />);
    await waitFor(() => expect(screen.getByText('保存')).toBeTruthy());
    await user.click(screen.getByText('保存'));
    await waitFor(() => {
      expect(mockedApi.setEditorConfig).toHaveBeenCalledWith({
        editor: 'vscode',
        custom_command: null,
        reuse_window: true,
      });
      expect(setSavedMsg).toHaveBeenCalled();
    });
  });

  it('checks current editor config on check click', async () => {
    const user = userEvent.setup();
    render(<ExternalEditorTab setSavedMsg={vi.fn()} />);
    await waitFor(() => expect(screen.getByText('检测')).toBeTruthy());

    await user.click(screen.getByText('检测'));

    await waitFor(() => {
      expect(mockedApi.checkExternalEditor).toHaveBeenCalledWith({
        editor: 'vscode',
        custom_command: null,
        reuse_window: true,
      });
      expect(screen.getByText('检测通过：将使用 VS Code')).toBeTruthy();
    });
  });

  it('shows check failures without saving', async () => {
    const user = userEvent.setup();
    mockedApi.checkExternalEditor.mockRejectedValue(new Error("editor command 'code' not found"));
    render(<ExternalEditorTab setSavedMsg={vi.fn()} />);
    await waitFor(() => expect(screen.getByText('检测')).toBeTruthy());

    await user.click(screen.getByText('检测'));

    await waitFor(() => {
      expect(screen.getByText(/editor command 'code' not found/)).toBeTruthy();
      expect(mockedApi.setEditorConfig).not.toHaveBeenCalled();
    });
  });
});
