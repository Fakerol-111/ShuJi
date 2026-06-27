import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { GlossaryTerm } from './GlossaryTerm';

export type ActivitySelection =
  | 'files'
  | 'stats'
  | 'context'
  | 'archives'
  | 'audit'
  | 'graph'
  | null;

interface ActivityBarProps {
  selected: ActivitySelection;
  onSelect: (selected: ActivitySelection) => void;
  pendingApprovalsCount?: number;
  beginnerMode?: boolean;
}

function FolderIcon({ active }: { active: boolean }) {
  return (
    <svg
      width="22"
      height="22"
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.5"
      strokeLinecap="round"
      strokeLinejoin="round"
      className={active ? 'text-ink-50' : ''}
    >
      <path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z" />
    </svg>
  );
}

function ChartIcon({ active }: { active: boolean }) {
  return (
    <svg
      width="22"
      height="22"
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.5"
      strokeLinecap="round"
      strokeLinejoin="round"
      className={active ? 'text-ink-50' : ''}
    >
      <line x1="18" y1="20" x2="18" y2="10" />
      <line x1="12" y1="20" x2="12" y2="4" />
      <line x1="6" y1="20" x2="6" y2="14" />
    </svg>
  );
}

function ScrollIcon({ active }: { active: boolean }) {
  return (
    <svg
      width="22"
      height="22"
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.5"
      strokeLinecap="round"
      strokeLinejoin="round"
      className={active ? 'text-ink-50' : ''}
    >
      <path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z" />
      <polyline points="14 2 14 8 20 8" />
      <line x1="16" y1="13" x2="8" y2="13" />
      <line x1="16" y1="17" x2="8" y2="17" />
    </svg>
  );
}

function ArchiveIcon({ active }: { active: boolean }) {
  return (
    <svg
      width="22"
      height="22"
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.5"
      strokeLinecap="round"
      strokeLinejoin="round"
      className={active ? 'text-ink-50' : ''}
    >
      <rect x="2" y="3" width="20" height="5" rx="1" />
      <path d="M4 8v11a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8" />
      <path d="M10 12h4" />
    </svg>
  );
}

function GraphIcon({ active }: { active: boolean }) {
  return (
    <svg
      width="22"
      height="22"
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.5"
      strokeLinecap="round"
      strokeLinejoin="round"
      className={active ? 'text-ink-50' : ''}
    >
      <circle cx="5" cy="12" r="2" />
      <circle cx="19" cy="5" r="2" />
      <circle cx="19" cy="19" r="2" />
      <line x1="7" y1="12" x2="17" y2="5" />
      <line x1="7" y1="12" x2="17" y2="19" />
      <line x1="17" y1="7" x2="17" y2="17" />
    </svg>
  );
}

function NewspaperIcon({ active }: { active: boolean }) {
  return (
    <svg
      width="22"
      height="22"
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.5"
      strokeLinecap="round"
      strokeLinejoin="round"
      className={active ? 'text-ink-50' : ''}
    >
      <path d="M4 22h16a2 2 0 0 0 2-2V4a2 2 0 0 0-2-2H8a2 2 0 0 0-2 2v16a2 2 0 0 1-4 0V6" />
      <line x1="10" y1="8" x2="18" y2="8" />
      <line x1="10" y1="12" x2="18" y2="12" />
      <line x1="10" y1="16" x2="14" y2="16" />
    </svg>
  );
}

type ItemDef = {
  id: Exclude<ActivitySelection, null>;
  icon: (active: boolean) => React.ReactNode;
  tooltip: string;
  glossaryTerm?: string;
  advanced?: boolean;
};

export default function ActivityBar({
  selected,
  onSelect,
  pendingApprovalsCount,
  beginnerMode = false,
}: ActivityBarProps) {
  const { t, i18n } = useTranslation();
  const isEn = i18n.language?.startsWith('en');
  const [advancedOpen, setAdvancedOpen] = useState(false);

  const ITEMS: ItemDef[] = [
    {
      id: 'files',
      icon: (a) => <FolderIcon active={a} />,
      tooltip: t('activityBar.files'),
      glossaryTerm: 'artifact',
    },
    {
      id: 'stats',
      icon: (a) => <ChartIcon active={a} />,
      tooltip: t('activityBar.tokens'),
      advanced: true,
    },
    {
      id: 'context',
      icon: (a) => <ScrollIcon active={a} />,
      tooltip: t('activityBar.context'),
      advanced: true,
    },
    {
      id: 'archives',
      icon: (a) => <ArchiveIcon active={a} />,
      tooltip: t('activityBar.checkpoints'),
      advanced: true,
    },
    {
      id: 'audit',
      icon: (a) => <NewspaperIcon active={a} />,
      tooltip: t('activityBar.audit'),
      advanced: true,
    },
    {
      id: 'graph',
      icon: (a) => <GraphIcon active={a} />,
      tooltip: t('activityBar.graph'),
      glossaryTerm: 'workflowGraph',
      advanced: true,
    },
  ];

  const visibleItems = ITEMS.filter((item) => {
    if (!beginnerMode) return true;
    if (!item.advanced) return true;
    return advancedOpen;
  });

  const renderItem = (item: ItemDef) => {
    const active = selected === item.id;
    const hasBadge = item.id === 'files' && (pendingApprovalsCount ?? 0) > 0;
    const label = item.glossaryTerm ? (
      <GlossaryTerm term={item.glossaryTerm}>{item.tooltip}</GlossaryTerm>
    ) : (
      item.tooltip
    );

    return (
      <button
        key={item.id}
        onClick={() => onSelect(active ? null : item.id)}
        aria-label={item.tooltip}
        className={`group relative w-full h-11 flex items-center justify-center transition-colors ${
          active ? 'bg-ink-800' : 'text-ink-500 hover:text-ink-200 hover:bg-ink-800/60'
        }`}
      >
        {active && (
          <span className="absolute left-0 top-1 bottom-1 w-0.5 bg-vermillion rounded-r" />
        )}
        {item.icon(active)}
        {hasBadge && (
          <span
            className="absolute top-1 right-1.5 w-2 h-2 bg-gold rounded-full animate-pulse"
            title={
              isEn
                ? `${pendingApprovalsCount} pending approval`
                : `${pendingApprovalsCount} 份朱批待批`
            }
          />
        )}
        <span className="absolute left-full ml-2 whitespace-nowrap bg-ink-800 text-ink-200 text-xs px-2 py-1 rounded border border-ink-700 shadow-lg opacity-0 group-hover:opacity-100 transition-opacity pointer-events-none z-50">
          {label}
        </span>
      </button>
    );
  };

  return (
    <div className="w-12 bg-ink-900 border-r border-ink-800 flex flex-col items-center py-2 shrink-0">
      {visibleItems.map(renderItem)}

      {beginnerMode && (
        <button
          type="button"
          onClick={() => setAdvancedOpen((v) => !v)}
          className="mt-auto w-full h-9 flex items-center justify-center text-caption text-ink-500 hover:text-ink-200 hover:bg-ink-800/60 border-t border-ink-800"
          title={t('activityBar.advancedView')}
          aria-expanded={advancedOpen}
        >
          {advancedOpen ? '▴' : '▾'}
        </button>
      )}
    </div>
  );
}
