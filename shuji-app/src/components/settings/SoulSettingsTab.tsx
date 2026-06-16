import { useTranslation } from 'react-i18next';
import { SettingsSection, SettingsAction, SettingsHint } from './SettingsPrimitives';

interface SoulSettingsTabProps {
  setSavedMsg: (msg: string) => void;
}

export default function SoulSettingsTab({ setSavedMsg }: SoulSettingsTabProps) {
  const { t } = useTranslation();
  return (
    <SettingsSection
      title={t('settings.soulManagement')}
      description={t('settings.soulManagement') + ' — .shuji/soul.md'}
    >
      <div className="flex gap-2 flex-wrap">
        <SettingsAction
          onClick={async () => {
            try {
              const { getSoulContent } = await import('../../api');
              const content = await getSoulContent();
              if (!content) {
                setSavedMsg(t('common.noData'));
                setTimeout(() => setSavedMsg(''), 2000);
                return;
              }
              await navigator.clipboard.writeText(content);
              setSavedMsg(t('common.saved'));
              setTimeout(() => setSavedMsg(''), 2000);
            } catch (e) {
              setSavedMsg(String(e));
            }
          }}
        >
          {t('common.export')}
        </SettingsAction>
        <SettingsAction
          variant="danger"
          onClick={async () => {
            try {
              const { clearSoul } = await import('../../api');
              await clearSoul();
              setSavedMsg(t('common.saved'));
              setTimeout(() => setSavedMsg(''), 2000);
            } catch (e) {
              setSavedMsg(String(e));
            }
          }}
        >
          {t('common.delete')}
        </SettingsAction>
      </div>
      <SettingsHint>{t('common.noData')}</SettingsHint>
    </SettingsSection>
  );
}
