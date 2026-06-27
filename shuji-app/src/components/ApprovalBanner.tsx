import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import type { ApprovalGateContext } from '../utils/approvalGate';

const DOC_TYPE_KEYS: Record<string, string> = {
  revw: 'chat.docType.revw',
  dsgn: 'chat.docType.dsgn',
  plan: 'chat.docType.plan',
};

interface ApprovalBannerProps {
  context: ApprovalGateContext;
  onView: () => void;
  onApprove: (comment?: string) => Promise<void>;
}

export default function ApprovalBanner({ context, onView, onApprove }: ApprovalBannerProps) {
  const { t } = useTranslation();
  const [approving, setApproving] = useState(false);
  const [error, setError] = useState('');
  const [comment, setComment] = useState('');

  if (!context.active || !context.docId) return null;

  const docTypeLabel = DOC_TYPE_KEYS[context.docType]
    ? t(DOC_TYPE_KEYS[context.docType])
    : context.docType;

  const handleApprove = async () => {
    setApproving(true);
    setError('');
    try {
      await onApprove(comment.trim() || undefined);
      setComment('');
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setApproving(false);
    }
  };

  return (
    <div
      className="shrink-0 border-b border-vermillion/30 bg-gradient-to-r from-vermillion/10 via-gold/5 to-surface-parchment px-4 py-3"
      role="status"
      aria-live="polite"
    >
      <div className="flex flex-wrap items-start gap-3 justify-between">
        <div className="min-w-0 flex-1 space-y-1">
          <div className="flex items-center gap-2 flex-wrap">
            <svg
              className="w-4 h-4 text-vermillion shrink-0"
              viewBox="0 0 24 24"
              fill="currentColor"
            >
              <path d="M12 2L15.09 8.26L22 9.27L17 14.14L18.18 21.02L12 17.77L5.82 21.02L7 14.14L2 9.27L8.91 8.26L12 2Z" />
            </svg>
            <span className="font-display text-ui font-bold text-vermillion">
              {t('approval.bannerTitle')}
            </span>
          </div>
          <p className="text-caption text-ink-700">{t('approval.bannerPaused')}</p>
          <div className="flex flex-wrap gap-x-4 gap-y-1 text-caption text-ink-600">
            <span>
              <span className="text-ink-400">{t('document.type')}：</span>
              <span className="font-medium">{docTypeLabel}</span>
            </span>
            <span className="font-mono">{context.docId}</span>
            {context.stepId && (
              <span>
                <span className="text-ink-400">{t('approval.pipelineStep')}：</span>
                {context.stepLabel || context.stepId}
              </span>
            )}
          </div>
          {context.nextStepLabel && (
            <p className="text-caption text-ink-500">
              {t('approval.afterApprove', { step: context.nextStepLabel })}
            </p>
          )}
          <p className="text-[11px] text-ink-400 leading-relaxed">
            {t('approval.notSatisfiedHint')}
          </p>
        </div>

        <div className="flex flex-col items-stretch sm:items-end gap-2 shrink-0">
          <input
            type="text"
            placeholder={t('document.imperialNote')}
            value={comment}
            onChange={(e) => setComment(e.target.value)}
            className="w-full sm:w-56 px-3 py-1.5 border border-fold rounded-lg text-body bg-surface-elevated"
          />
          <div className="flex flex-wrap gap-2">
            <button
              type="button"
              onClick={onView}
              className="px-3 py-1.5 rounded-lg border border-fold bg-surface-elevated text-ui font-medium text-ink-700 hover:border-gold/40 transition-colors"
            >
              {t('approval.viewDocument')}
            </button>
            <button
              type="button"
              onClick={handleApprove}
              disabled={approving}
              className="px-3 py-1.5 rounded-lg bg-jade text-white text-ui font-bold hover:bg-jade/90 disabled:opacity-50 transition-colors"
            >
              {approving ? t('common.loading') : t('document.approve')}
            </button>
          </div>
          {error && <p className="text-caption text-vermillion max-w-xs text-right">{error}</p>}
        </div>
      </div>
    </div>
  );
}
