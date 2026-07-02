import { useTranslation } from 'react-i18next';

export default function ApprovalBanner({
  approving,
  approvalError,
  comment,
  onCommentChange,
  onApprove,
}: {
  approving: boolean;
  approvalError: string;
  comment: string;
  onCommentChange: (v: string) => void;
  onApprove: () => void;
}) {
  const { t } = useTranslation();

  return (
    <div className="mb-4 rounded-lg border border-vermillion/40 bg-vermillion/5 px-3 py-3">
      <div className="flex items-center justify-between gap-3 flex-wrap">
        <div className="min-w-0">
          <h3 className="font-display text-sm font-bold text-ink-900">
            {t('document.pendingApproval')}
          </h3>
          <p className="text-caption text-ink-600 mt-0.5">{t('document.approvalRequired')}</p>
        </div>
        <button
          onClick={onApprove}
          disabled={approving}
          className="bg-jade hover:bg-jade/80 text-white text-ui font-bold px-3 py-1.5 rounded transition disabled:opacity-50 shrink-0"
        >
          {approving ? t('common.loading') : t('document.approve')}
        </button>
      </div>
      <div className="mt-2">
        <input
          type="text"
          placeholder={t('document.imperialNote')}
          value={comment}
          onChange={(e) => onCommentChange(e.target.value)}
          className="w-full min-w-0 px-3 py-1.5 border border-fold rounded text-body bg-surface-paper"
        />
      </div>
      <p className="text-[11px] text-ink-400 mt-2 leading-relaxed">
        {t('approval.notSatisfiedHint')}
      </p>
      {approvalError && <p className="text-caption text-vermillion mt-1">{approvalError}</p>}
    </div>
  );
}
