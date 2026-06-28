import { useTranslation } from 'react-i18next';

export interface TabInfo {
  path: string;
  label: string;
  initialView?: 'content' | 'diff' | 'lineage';
}

interface TabBarProps {
  tabs: TabInfo[];
  activeIndex: number;
  onSelect: (index: number) => void;
  onClose: (index: number) => void;
}

export default function TabBar({ tabs, activeIndex, onSelect, onClose }: TabBarProps) {
  const { t } = useTranslation();
  if (tabs.length === 0) return null;

  return (
    <div className="flex items-center border-b border-fold bg-surface-parchment shrink-0 overflow-x-auto">
      {tabs.map((tab, i) => {
        const isActive = i === activeIndex;
        return (
          <button
            key={tab.path}
            onClick={() => onSelect(i)}
            role="tab"
            aria-selected={isActive}
            className={`group flex items-center gap-1 px-3 py-1.5 text-caption font-mono cursor-pointer border-r border-fold whitespace-nowrap shrink-0 transition-colors ${
              isActive
                ? 'bg-surface-paper text-ink-900 border-b-2 border-b-vermillion mb-[-1px]'
                : 'bg-surface-parchment text-ink-500 hover:text-ink-700 hover:bg-ink-100/50'
            }`}
          >
            <span className="truncate max-w-[160px]">{tab.label}</span>
            <button
              type="button"
              onClick={(e) => {
                e.stopPropagation();
                onClose(i);
              }}
              className={`ml-1 w-3.5 h-3.5 flex items-center justify-center rounded text-[10px] hover:bg-ink-200/60 hover:text-ink-900 transition-opacity text-ink-400 shrink-0 ${
                isActive ? 'opacity-100' : 'opacity-60 group-hover:opacity-100'
              }`}
              title={t('common.close')}
              aria-label={`${t('common.close')} ${tab.label}`}
            >
              ✕
            </button>
          </button>
        );
      })}
    </div>
  );
}
