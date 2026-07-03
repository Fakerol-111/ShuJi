import type { ReactNode } from 'react';
import { useTranslation } from 'react-i18next';
import type { SettingsCategory } from '../../pages/SettingsPage';

interface SettingsSidebarProps {
  activeCategory: SettingsCategory;
  onSelect: (category: SettingsCategory) => void;
}

function SlidersIcon() {
  return (
    <svg
      width="18"
      height="18"
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.5"
      strokeLinecap="round"
      strokeLinejoin="round"
    >
      <line x1="4" y1="21" x2="4" y2="14" />
      <line x1="4" y1="10" x2="4" y2="3" />
      <line x1="12" y1="21" x2="12" y2="12" />
      <line x1="12" y1="8" x2="12" y2="3" />
      <line x1="20" y1="21" x2="20" y2="16" />
      <line x1="20" y1="12" x2="20" y2="3" />
      <circle cx="4" cy="12" r="2" />
      <circle cx="12" cy="10" r="2" />
      <circle cx="20" cy="14" r="2" />
    </svg>
  );
}

function FileTextIcon() {
  return (
    <svg
      width="18"
      height="18"
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.5"
      strokeLinecap="round"
      strokeLinejoin="round"
    >
      <path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z" />
      <polyline points="14 2 14 8 20 8" />
      <line x1="16" y1="13" x2="8" y2="13" />
      <line x1="16" y1="17" x2="8" y2="17" />
    </svg>
  );
}

function StarIcon() {
  return (
    <svg
      width="18"
      height="18"
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.5"
      strokeLinecap="round"
      strokeLinejoin="round"
    >
      <polygon points="12 2 15.09 8.26 22 9.27 17 14.14 18.18 21.02 12 17.77 5.82 21.02 7 14.14 2 9.27 8.91 8.26 12 2" />
    </svg>
  );
}

function CodeIcon() {
  return (
    <svg
      width="18"
      height="18"
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.5"
      strokeLinecap="round"
      strokeLinejoin="round"
    >
      <polyline points="16 18 22 12 16 6" />
      <polyline points="8 6 2 12 8 18" />
    </svg>
  );
}

function PaletteIcon() {
  return (
    <svg
      width="18"
      height="18"
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.5"
      strokeLinecap="round"
      strokeLinejoin="round"
    >
      <circle cx="13.5" cy="6.5" r="0.5" fill="currentColor" />
      <circle cx="17.5" cy="10.5" r="0.5" fill="currentColor" />
      <circle cx="8.5" cy="7.5" r="0.5" fill="currentColor" />
      <circle cx="6.5" cy="12.5" r="0.5" fill="currentColor" />
      <path d="M12 2C6.5 2 2 6.5 2 12s4.5 10 10 10c.93 0 1.5-.67 1.5-1.5 0-.39-.15-.74-.39-1.01-.23-.26-.38-.61-.38-1 0-.83.67-1.5 1.5-1.5H16c3.31 0 6-2.69 6-6 0-5.5-4.5-10-10-10z" />
    </svg>
  );
}

const NAV_ITEMS: {
  key: SettingsCategory;
  icon: () => ReactNode;
  labelKey: string;
  basic: boolean;
}[] = [
  { key: 'service', icon: SlidersIcon, labelKey: 'settings.serviceConfig', basic: true },
  { key: 'appearance', icon: PaletteIcon, labelKey: 'settings.appearance', basic: true },
  { key: 'context', icon: FileTextIcon, labelKey: 'settings.contextWindow', basic: false },
  { key: 'soul', icon: StarIcon, labelKey: 'settings.soulManagement', basic: false },
  { key: 'externalEditor', icon: CodeIcon, labelKey: 'settings.externalEditor', basic: false },
];

export default function SettingsSidebar({ activeCategory, onSelect }: SettingsSidebarProps) {
  const { t } = useTranslation();
  const basicItems = NAV_ITEMS.filter((i) => i.basic);
  const advancedItems = NAV_ITEMS.filter((i) => !i.basic);

  return (
    <nav className="w-52 shrink-0 bg-surface-parchment border-r border-fold flex flex-col">
      <div className="h-9 px-4 border-b border-fold flex items-center font-display text-ui font-semibold text-ink-700">
        {t('common.settings')}
      </div>
      <div className="p-2 space-y-0.5 flex-1 overflow-y-auto">
        <div className="text-[10px] font-semibold uppercase tracking-wider text-ink-400 px-3 pb-1 pt-2">
          {t('settings.basic')}
        </div>
        {basicItems.map((item) => renderItem(item))}
        <div className="text-[10px] font-semibold uppercase tracking-wider text-ink-400 px-3 pb-1 pt-4">
          {t('settings.advanced')}
        </div>
        {advancedItems.map((item) => renderItem(item))}
      </div>
    </nav>
  );

  function renderItem(item: (typeof NAV_ITEMS)[number]) {
    const Icon = item.icon;
    const active = activeCategory === item.key;
    return (
      <button
        key={item.key}
        onClick={() => onSelect(item.key)}
        className={`w-full flex items-center gap-2.5 px-3 py-2 text-sm rounded-lg transition-colors text-left ${
          active
            ? 'bg-ink-900 text-ink-50 shadow-sm'
            : 'text-ink-700 hover:text-ink-900 hover:bg-ink-100'
        }`}
      >
        <Icon />
        <span className="font-medium">{t(item.labelKey)}</span>
      </button>
    );
  }
}
