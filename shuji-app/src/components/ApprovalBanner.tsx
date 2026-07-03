import { useState, type FormEvent } from 'react';
import { useTranslation } from 'react-i18next';
import type { ApprovalGateContext } from '../utils/approvalGate';

const DOC_TYPE_KEYS: Record<string, string> = {
  revw: 'chat.docType.revw',
  dsgn: 'chat.docType.dsgn',
  plan: 'chat.docType.plan',
};

const WHY_KEYS: Record<string, string> = {
  revw: 'approval.whyReview',
  dsgn: 'approval.whyDesign',
  plan: 'approval.whyPlan',
};

interface ApprovalBannerProps {
  context: ApprovalGateContext;
  onView: () => void;
  onApprove: (comment?: string) => Promise<void>;
  onStop?: () => void;
  resuming?: boolean;
}

export default function ApprovalBanner({
  context,
  onView,
  onApprove,
  onStop,
  resuming = false,
}: ApprovalBannerProps) {
  const { t } = useTranslation();
  const [approving, setApproving] = useState(false);
  const [error, setError] = useState('');
  const [comment, setComment] = useState('');

  if (!context.active || !context.docId) return null;

  const docTypeLabel = DOC_TYPE_KEYS[context.docType]
    ? t(DOC_TYPE_KEYS[context.docType])
    : context.docType;
  const whyKey = WHY_KEYS[context.docType] ?? 'approval.whyDefault';
  const isRevw = context.docType === 'revw';

  const handleApprove = async (e: FormEvent) => {
    e.preventDefault();
    if (resuming) return;
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
      <form onSubmit={handleApprove}>
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
              {resuming && (
                <span className="text-caption text-amber font-medium animate-pulse">
                  {t('approval.resuming')}
                </span>
              )}
            </div>
            <p className="text-caption text-ink-700">{t('approval.bannerPaused')}</p>
            <p className="text-caption text-ink-600 leading-relaxed">{t(whyKey)}</p>
            <div className="flex flex-wrap gap-x-4 gap-y-1 text-caption text-ink-600">
              <span>
                <span className="text-ink-400">{t('document.type')}：</span>
                <span className="font-medium">{docTypeLabel}</span>
              </span>
              <span>
                <span className="text-ink-400">{t('approval.pendingDoc')}：</span>
                <span className="font-mono">{context.docId}</span>
              </span>
              {context.stepId && (
                <span>
                  <span className="text-ink-400">{t('approval.pipelineStep')}：</span>
                  {context.stepLabel || context.stepId}
                </span>
              )}
            </div>
            {context.nextStepLabel && (
              <p className="text-caption text-jade font-medium">
                {t('approval.afterApprove', { step: context.nextStepLabel })}
              </p>
            )}
            <p className="text-[11px] text-ink-400 leading-relaxed">
              {t(isRevw ? 'approval.rejectConsequence' : 'approval.rejectConsequenceSimple')}
            </p>
          </div>

          <div className="flex flex-col items-stretch sm:items-end gap-2 shrink-0">
            <input
              type="text"
              placeholder={t('document.imperialNote')}
              value={comment}
              onChange={(e) => setComment(e.target.value)}
              disabled={resuming}
              className="w-full sm:w-56 px-3 py-1.5 border border-fold rounded-lg text-body bg-surface-elevated disabled:opacity-50"
            />
            <div className="flex flex-wrap gap-2">
              <button
                type="button"
                onClick={onView}
                disabled={resuming}
                className="px-3 py-1.5 rounded-lg border border-fold bg-surface-elevated text-ui font-medium text-ink-700 hover:border-gold/40 transition-colors disabled:opacity-50"
              >
                {t('approval.viewDocument')}
              </button>
              {onStop && (
                <button
                  type="button"
                  onClick={onStop}
                  disabled={resuming}
                  className="px-3 py-1.5 rounded-lg border border-vermillion/30 text-vermillion text-ui font-medium hover:bg-vermillion/10 transition-colors disabled:opacity-50"
                >
                  {t('chat.stopAllDepts')}
                </button>
              )}
              <button
                type="submit"
                disabled={approving || resuming}
                className="px-3 py-1.5 rounded-lg bg-jade text-white text-ui font-bold hover:bg-jade/90 disabled:opacity-50 transition-colors"
              >
                {approving ? t('common.loading') : t('document.approve')}
              </button>
            </div>
            {error && <p className="text-caption text-vermillion max-w-xs text-right">{error}</p>}
          </div>
        </div>
      </form>
    </div>
  );
}
