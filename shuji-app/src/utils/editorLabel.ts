import type { TFunction } from 'i18next';
import type { EditorConfig } from '../api';

export function editorKindLabel(config: EditorConfig, t: TFunction): string {
  if (config.editor === 'custom') {
    const cmd = config.custom_command?.trim();
    if (cmd) {
      const base = cmd.replace(/^.*[\\/]/, '').replace(/\.(cmd|exe|bat|com)$/i, '');
      if (base) return base;
    }
    return t('settings.externalEditor.custom');
  }
  return t(`settings.externalEditor.${config.editor}`);
}

export function openInEditorLabel(config: EditorConfig, t: TFunction): string {
  return t('editor.openInEditor', { editor: editorKindLabel(config, t) });
}

export function openLineInEditorLabel(config: EditorConfig, t: TFunction): string {
  return t('editor.openLineInEditor', { editor: editorKindLabel(config, t) });
}

export function openProjectInEditorLabel(config: EditorConfig, t: TFunction): string {
  return t('editor.openProjectInEditor', { editor: editorKindLabel(config, t) });
}
