import { useTranslation } from 'react-i18next';
import DocTree from './DocTree';
import TokenPanel from './TokenPanel';
import ContextPanel from './ContextPanel';
import CheckpointPanel from './CheckpointPanel';
import AuditPanel from './AuditPanel';
import type { ActivitySelection } from './ActivityBar';

const headerKey: Record<string, string> = {
  files: 'sidebar.directory',
  stats: 'sidebar.tokens',
  context: 'sidebar.context',
  archives: 'sidebar.checkpoints',
  audit: 'sidebar.audit',
};

function panel(
  mode: Exclude<ActivitySelection, null>,
  projectDir: string,
  selectedDoc: string | null,
  onDocSelect: (path: string) => void,
  onShowDiff?: (path: string) => void
) {
  switch (mode) {
    case 'files':
      return <DocTree projectDir={projectDir} selectedDoc={selectedDoc} onSelect={onDocSelect} />;
    case 'stats':
      return <TokenPanel />;
    case 'context':
      return <ContextPanel />;
    case 'archives':
      return <CheckpointPanel />;
    case 'audit':
      return (
        <AuditPanel projectDir={projectDir} onDocSelect={onDocSelect} onShowDiff={onShowDiff} />
      );
  }
}

interface SidebarProps {
  mode: Exclude<ActivitySelection, null>;
  projectDir: string;
  selectedDoc: string | null;
  onDocSelect: (path: string) => void;
  onShowDiff?: (path: string) => void;
}

export default function Sidebar({
  mode,
  projectDir,
  selectedDoc,
  onDocSelect,
  onShowDiff,
}: SidebarProps) {
  const { t } = useTranslation();
  return (
    <aside className="w-60 bg-surface-parchment border-r border-fold shrink-0 flex flex-col min-h-0">
      <div className="h-9 px-3 border-b border-fold flex items-center font-display text-ui font-semibold text-ink-700">
        {t(headerKey[mode] || mode, headerKey[mode] || mode)}
      </div>
      <div className="flex-1 min-h-0 overflow-y-auto">
        {panel(mode, projectDir, selectedDoc, onDocSelect, onShowDiff)}
      </div>
    </aside>
  );
}
