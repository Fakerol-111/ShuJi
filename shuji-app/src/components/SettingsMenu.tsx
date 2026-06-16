import { useTranslation } from 'react-i18next';
import LangSwitcher from './LangSwitcher';

interface SettingsMenuProps {
  onOpenSettings: () => void;
}

export default function SettingsMenu({ onOpenSettings }: SettingsMenuProps) {
  const { t } = useTranslation();
  return (
    <div className="flex items-center gap-1">
      <LangSwitcher />
      <button
        onClick={onOpenSettings}
        className="text-xs px-2 py-1 text-ink-400 hover:text-ink-100 hover:bg-ink-800 rounded"
      >
        {t('common.edit')}
      </button>
    </div>
  );
}
