import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import LangSwitcher from './LangSwitcher';
import { exportDiagnostics } from '../api';

interface SettingsMenuProps {
  onOpenSettings: () => void;
}

export default function SettingsMenu({ onOpenSettings }: SettingsMenuProps) {
  const { t } = useTranslation();
  const [exporting, setExporting] = useState(false);

  const handleExport = async () => {
    setExporting(true);
    try {
      const bundle = await exportDiagnostics();
      const blob = new Blob([bundle], { type: 'application/json' });
      const url = URL.createObjectURL(blob);
      const a = document.createElement('a');
      a.href = url;
      a.download = `shuji-diagnostics-${Date.now()}.json`;
      a.click();
      URL.revokeObjectURL(url);
    } catch (err) {
      console.error('Diagnostics export failed:', err);
    } finally {
      setExporting(false);
    }
  };

  return (
    <div className="flex items-center gap-1">
      <LangSwitcher />
      <button
        onClick={handleExport}
        disabled={exporting}
        className="text-xs px-2 py-1 text-ink-400 hover:text-ink-100 hover:bg-ink-800 rounded disabled:opacity-50"
        title={t('common.exportDiagnostics') ?? 'Export diagnostics'}
      >
        {exporting ? t('common.exporting') || '...' : t('common.exportDiagnostics') || 'Export'}
      </button>
      <button
        onClick={onOpenSettings}
        className="text-xs px-2 py-1 text-ink-400 hover:text-ink-100 hover:bg-ink-800 rounded"
      >
        {t('common.settings')}
      </button>
    </div>
  );
}
