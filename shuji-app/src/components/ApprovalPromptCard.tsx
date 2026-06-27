import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { docIdToPath } from '../utils/docPath';
import type { ApprovalGateContext } from '../utils/approvalGate';

interface ApprovalPromptCardProps {
  docPaths: string[];
  gateContext: ApprovalGateContext;
  onSelect: (path: string) => void;
  onApprove?: (docId: string, comment?: string) => Promise<void>;
}

export default function ApprovalPromptCard({
  docPaths,
  gateContext,
  onSelect,
  onApprove,
}: ApprovalPromptCardProps) {
  const { t } = useTranslation();
  const [approvingId, setApprovingId] = useState<string | null>(null);
  const [error, setError] = useState('');

  if (docPaths.length === 0) return null;

  const primaryId = gateContext.docId ?? docPaths[0];
  const docTypeLabel = gateContext.docType
    ? t(`chat.docType.${gateContext.docType}`, gateContext.docType)
    : 'revw';

  const handleApprove = async (docId: string) => {
    if (!onApprove) return;
    setApprovingId(docId);
    setError('');
    try {
      await onApprove(docId);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setApprovingId(null);
    }
  };

  return (
    <div className="flex-1 flex flex-col items-center justify-center p-6 text-center overflow-y-auto">
      <div className="rounded-xl border border-vermillion/30 bg-surface-elevated p-6 max-w-md w-full shadow-sm text-left">
        <div className="flex items-center gap-2 mb-2">
          <svg className="w-4 h-4 text-vermillion shrink-0" viewBox="0 0 24 24" fill="currentColor">
            <path d="M12 2L15.09 8.26L22 9.27L17 14.14L18.18 21.02L12 17.77L5.82 21.02L7 14.14L2 9.27L8.91 8.26L12 2Z" />
          </svg>
          <h3 className="font-display text-sm font-bold text-ink-900">
            {t('document.pendingApproval')}
          </h3>
        </div>
        <p className="text-caption text-ink-600 mb-3">{t('approval.bannerPaused')}</p>

        {gateContext.stepId && (
          <p className="text-caption text-ink-500 mb-2">
            {t('approval.pipelineStep')}：{gateContext.stepLabel || gateContext.stepId}
          </p>
        )}
        {gateContext.nextStepLabel && (
          <p className="text-caption text-ink-500 mb-3">
            {t('approval.afterApprove', { step: gateContext.nextStepLabel })}
          </p>
        )}

        <div className="space-y-2 mb-4">
          {docPaths.slice(0, 5).map((docId) => {
            const path = docId.startsWith('.shuji/') ? docId : docIdToPath(docId);
            const isPrimary = docId === primaryId;
            return (
              <div
                key={docId}
                className={`rounded-lg border px-3 py-2 ${
                  isPrimary
                    ? 'border-vermillion/30 bg-vermillion/5'
                    : 'border-fold bg-surface-paper'
                }`}
              >
                <div className="flex items-center justify-between gap-2 flex-wrap">
                  <div className="min-w-0">
                    <div className="text-[10px] text-ink-400 uppercase tracking-wide">
                      {docTypeLabel}
                    </div>
                    <button
                      type="button"
                      onClick={() => onSelect(path)}
                      className="font-mono text-ui text-ink-800 hover:text-vermillion truncate max-w-full text-left"
                    >
                      {docId}
                    </button>
                  </div>
                  <div className="flex gap-1.5 shrink-0">
                    <button
                      type="button"
                      onClick={() => onSelect(path)}
                      className="text-caption px-2 py-1 rounded border border-fold hover:bg-ink-100/50"
                    >
                      {t('approval.viewDocument')}
                    </button>
                    {onApprove && (
                      <button
                        type="button"
                        onClick={() => handleApprove(docId)}
                        disabled={approvingId === docId}
                        className="text-caption px-2 py-1 rounded bg-jade text-white font-semibold disabled:opacity-50"
                      >
                        {approvingId === docId ? t('common.loading') : t('document.approve')}
                      </button>
                    )}
                  </div>
                </div>
              </div>
            );
          })}
          {docPaths.length > 5 && (
            <p className="text-caption text-ink-400">
              +{docPaths.length - 5} {t('approval.morePending')}
            </p>
          )}
        </div>

        <p className="text-[11px] text-ink-400 leading-relaxed border-t border-fold/50 pt-3">
          {t('approval.notSatisfiedHint')}
        </p>
        {error && <p className="text-caption text-vermillion mt-2">{error}</p>}
      </div>
    </div>
  );
}
