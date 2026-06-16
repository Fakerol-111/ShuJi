import { useTranslation } from 'react-i18next';
import { docIdToPath } from '../utils/docPath';

interface ApprovalPromptCardProps {
  docPaths: string[];
  projectDir: string;
  onSelect: (path: string) => void;
}

export default function ApprovalPromptCard({ docPaths, onSelect }: ApprovalPromptCardProps) {
  const { t } = useTranslation();
  if (docPaths.length === 0) return null;

  return (
    <div className="flex-1 flex flex-col items-center justify-center p-6 text-center overflow-y-auto">
      <div className="rounded-xl border border-vermillion/30 bg-surface-elevated p-6 max-w-sm w-full shadow-sm">
        <div className="flex items-center gap-2 justify-center mb-3">
          <svg className="w-4 h-4 text-vermillion shrink-0" viewBox="0 0 24 24" fill="currentColor">
            <path d="M12 2L15.09 8.26L22 9.27L17 14.14L18.18 21.02L12 17.77L5.82 21.02L7 14.14L2 9.27L8.91 8.26L12 2Z" />
          </svg>
          <h3 className="font-display text-sm font-bold text-ink-900">{t('document.pendingApproval')}</h3>
        </div>
        <p className="text-caption text-ink-600 mb-4">{t('document.approvalRequired')}</p>
        <div className="space-y-2">
          {docPaths.slice(0, 5).map((docId) => {
            const path = docId.startsWith('.shuji/') ? docId : docIdToPath(docId);
            return (
              <button
                key={docId}
                onClick={() => onSelect(path)}
                className="w-full text-left px-3 py-2 rounded-lg border border-fold bg-surface-paper hover:bg-ink-100/50 transition-colors text-ui font-mono text-ink-700"
              >
                {path.split('/').pop()?.replace(/\.md$/, '') || docId}
              </button>
            );
          })}
          {docPaths.length > 5 && (
            <p className="text-caption text-ink-400">{t('common.noRecords')} {docPaths.length - 5}</p>
          )}
        </div>
      </div>
    </div>
  );
}
