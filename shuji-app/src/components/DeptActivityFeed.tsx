import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { useDeptEvents } from '../hooks/useDeptEvents';
import DeptActivityCard from './DeptActivityCard';
import type { DeptLogEntry } from '../types';

interface DeptActivityFeedProps {
  onDocClick?: (docPath: string) => void;
}

const MAX_VISIBLE = 20;

function dedupeEntries(entries: DeptLogEntry[]): DeptLogEntry[] {
  return entries.filter((entry, i) => {
    if (i === 0) return true;
    const prev = entries[i - 1];
    return !(prev.dept === entry.dept && prev.action === entry.action);
  });
}

export default function DeptActivityFeed({ onDocClick }: DeptActivityFeedProps) {
  const { t } = useTranslation();
  const [showAll, setShowAll] = useState(false);
  const { logEntries } = useDeptEvents();

  const deduped = dedupeEntries(logEntries);

  if (deduped.length === 0) {
    return <div className="px-3 py-4 text-center text-caption text-ink-400">{t('deptActivity.noActivity')}</div>;
  }

  const visible = showAll ? deduped : deduped.slice(-MAX_VISIBLE);
  const hidden = deduped.length - MAX_VISIBLE;

  return (
    <div className="divide-y divide-ink-100/50">
      {!showAll && hidden > 0 && (
        <button
          onClick={() => setShowAll(true)}
          className="w-full px-3 py-1.5 text-[10px] text-ink-400 hover:text-ink-600 text-center transition-colors"
        >
          {t('deptActivity.showEarlier', { count: hidden })}
        </button>
      )}
      {visible.map((entry) => (
        <DeptActivityCard key={logEntries.indexOf(entry)} entry={entry} onDocClick={onDocClick} />
      ))}
    </div>
  );
}
