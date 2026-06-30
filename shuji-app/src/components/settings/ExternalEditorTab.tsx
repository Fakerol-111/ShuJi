import { useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import {
  checkExternalEditor,
  getEditorConfig,
  setEditorConfig,
  type EditorConfig,
} from '../../api';
import { formatError } from '../../utils/error';
import { editorKindLabel } from '../../utils/editorLabel';
import {
  SettingsSection,
  SettingsChip,
  SettingsField,
  SettingsToggle,
  SettingsSaveButton,
  SettingsHint,
  SettingsAction,
} from './SettingsPrimitives';

interface ExternalEditorTabProps {
  setSavedMsg: (msg: string) => void;
}

type EditorChoice = EditorConfig['editor'];

export default function ExternalEditorTab({ setSavedMsg }: ExternalEditorTabProps) {
  const { t } = useTranslation();
  const [editor, setEditor] = useState<EditorChoice>('vscode');
  const [customCommand, setCustomCommand] = useState('');
  const [reuseWindow, setReuseWindow] = useState(true);
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [checking, setChecking] = useState(false);
  const [checkMsg, setCheckMsg] = useState('');
  const [checkOk, setCheckOk] = useState(false);

  useEffect(() => {
    getEditorConfig()
      .then((cfg) => {
        setEditor(cfg.editor);
        setCustomCommand(cfg.custom_command ?? '');
        setReuseWindow(cfg.reuse_window);
      })
      .catch((e) => console.error(formatError(e)))
      .finally(() => setLoading(false));
  }, []);

  const currentConfig = (): EditorConfig => ({
    editor,
    custom_command: editor === 'custom' ? customCommand.trim() || null : null,
    reuse_window: reuseWindow,
  });

  const handleSave = async () => {
    setSaving(true);
    try {
      await setEditorConfig(currentConfig());
      setSavedMsg(t('common.saved'));
    } catch (e) {
      setSavedMsg(formatError(e));
    } finally {
      setSaving(false);
    }
  };

  const handleCheck = async () => {
    setChecking(true);
    setCheckMsg('');
    setCheckOk(false);
    try {
      await checkExternalEditor(currentConfig());
      setCheckOk(true);
      setCheckMsg(t('editor.checkSuccess', { editor: editorKindLabel(currentConfig(), t) }));
    } catch (e) {
      setCheckMsg(formatError(e));
    } finally {
      setChecking(false);
    }
  };

  if (loading) {
    return <p className="text-sm text-ink-400">{t('common.loading')}</p>;
  }

  return (
    <div className="space-y-6">
      <SettingsSection
        title={t('settings.externalEditor')}
        description={t('settings.externalEditorDesc')}
      >
        <div className="flex gap-2 flex-wrap">
          {(['vscode', 'cursor', 'trae', 'zed', 'sublime', 'jetbrains', 'custom'] as const).map(
            (choice) => (
              <SettingsChip
                key={choice}
                selected={editor === choice}
                onClick={() => setEditor(choice)}
              >
                {t(`settings.externalEditor.${choice}`)}
              </SettingsChip>
            )
          )}
        </div>
        <SettingsHint>{t('settings.externalEditorHint')}</SettingsHint>
      </SettingsSection>

      {editor === 'custom' && (
        <SettingsSection title={t('settings.externalEditor.customCommand')} divider>
          <SettingsField
            label={t('settings.externalEditor.customCommand')}
            hint={t('settings.externalEditor.customCommandHint')}
            value={customCommand}
            onChange={(e) => setCustomCommand(e.target.value)}
            placeholder={t('settings.externalEditor.customCommandPlaceholder')}
          />
        </SettingsSection>
      )}

      <SettingsSection title={t('settings.externalEditor.reuseWindow')} divider>
        <SettingsToggle
          checked={reuseWindow}
          onChange={setReuseWindow}
          label={t('settings.externalEditor.reuseWindow')}
        />
        <SettingsHint>{t('settings.externalEditor.reuseWindowHint')}</SettingsHint>
      </SettingsSection>

      <div className="flex items-center gap-3 flex-wrap">
        <SettingsSaveButton onClick={handleSave} disabled={saving || checking}>
          {saving ? t('common.loading') : t('common.save')}
        </SettingsSaveButton>
        <SettingsAction onClick={handleCheck} disabled={saving || checking}>
          {checking ? t('common.loading') : t('settings.externalEditor.check')}
        </SettingsAction>
      </div>

      {checkMsg && (
        <p className={`text-xs ${checkOk ? 'text-jade' : 'text-vermillion'}`}>{checkMsg}</p>
      )}
    </div>
  );
}
