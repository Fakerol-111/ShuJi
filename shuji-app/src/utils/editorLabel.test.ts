import { describe, it, expect } from 'vitest';
import i18n from '../i18n/config';
import { editorKindLabel, openInEditorLabel } from './editorLabel';
import type { EditorConfig } from '../api';

describe('editorLabel', () => {
  it('uses localized preset editor names', () => {
    const config: EditorConfig = { editor: 'cursor', custom_command: null, reuse_window: true };
    expect(editorKindLabel(config, i18n.t)).toBe('Cursor');
    expect(openInEditorLabel(config, i18n.t)).toBe('用 Cursor 打开');
  });

  it('derives custom editor label from command basename', () => {
    const config: EditorConfig = {
      editor: 'custom',
      custom_command: 'C:\\Tools\\MyEditor.exe',
      reuse_window: true,
    };
    expect(editorKindLabel(config, i18n.t)).toBe('MyEditor');
    expect(openInEditorLabel(config, i18n.t)).toBe('用 MyEditor 打开');
  });
});
