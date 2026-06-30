import { useEffect, useState } from 'react';
import { getEditorConfig, type EditorConfig } from '../api';

const DEFAULT_EDITOR_CONFIG: EditorConfig = {
  editor: 'vscode',
  custom_command: null,
  reuse_window: true,
};

export function useEditorConfig() {
  const [config, setConfig] = useState<EditorConfig>(DEFAULT_EDITOR_CONFIG);

  useEffect(() => {
    getEditorConfig()
      .then(setConfig)
      .catch(() => setConfig(DEFAULT_EDITOR_CONFIG));
  }, []);

  return config;
}
