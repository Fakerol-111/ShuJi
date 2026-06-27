import { useTranslation } from 'react-i18next';
import type { ChatDocument } from '../types';
import { docIdToPath } from '../utils/docPath';

const DOC_TYPE_KEYS: Record<string, string> = {
  dsgn: 'chat.docType.dsgn',
  plan: 'chat.docType.plan',
  pdsg: 'chat.docType.pdsg',
  ddtl: 'chat.docType.ddtl',
  revw: 'chat.docType.revw',
  task: 'chat.docType.task',
  ctrt: 'chat.docType.ctrt',
  rprt: 'chat.docType.rprt',
  anls: 'chat.docType.anls',
  reqs: 'chat.docType.reqs',
};

function docTypeLabel(t: (key: string) => string, docType: string): string {
  const key = DOC_TYPE_KEYS[docType];
  return key ? t(key) : docType;
}

function statusBadge(
  t: (key: string) => string,
  status: string,
  docType: string
): { label: string; className: string } | null {
  if (status === 'in_review' || (docType === 'revw' && status !== 'approved')) {
    return {
      label: t('chat.docPendingApproval'),
      className: 'bg-gold/15 text-gold border-gold/30',
    };
  }
  if (status === 'approved') {
    return {
      label: t('chat.docApproved'),
      className: 'bg-jade/10 text-jade border-jade/30',
    };
  }
  if (status === 'rejected') {
    return {
      label: t('chat.docRejected'),
      className: 'bg-vermillion/10 text-vermillion border-vermillion/30',
    };
  }
  return null;
}

export function ChatDocumentCards({
  documents,
  onDocumentClick,
}: {
  documents: ChatDocument[];
  onDocumentClick?: (path: string) => void;
}) {
  const { t } = useTranslation();
  if (documents.length === 0 || !onDocumentClick) return null;

  return (
    <div className="mt-2 space-y-1.5">
      <div className="text-caption text-ink-500 font-display">{t('chat.attachedDocuments')}</div>
      {documents.map((doc) => {
        const path = doc.path ?? docIdToPath(doc.id);
        const badge = statusBadge(t, doc.status, doc.doc_type);
        return (
          <button
            key={doc.id}
            type="button"
            onClick={() => onDocumentClick(path)}
            className="w-full text-left rounded-lg border border-fold bg-surface-parchment/80 px-3 py-2 hover:border-gold/40 hover:bg-gold-light/20 transition-colors"
          >
            <div className="flex items-center gap-2 flex-wrap">
              <span className="text-[10px] font-mono px-1.5 py-0.5 rounded bg-ink-100 text-ink-600">
                {docTypeLabel(t, doc.doc_type)}
              </span>
              <span className="font-mono text-caption text-ink-700">{doc.id}</span>
              {badge && (
                <span
                  className={`text-[10px] font-semibold px-1.5 py-0.5 rounded border ${badge.className}`}
                >
                  {badge.label}
                </span>
              )}
            </div>
            {doc.title && <div className="text-ui text-ink-800 mt-1 line-clamp-2">{doc.title}</div>}
            <div className="text-[10px] text-ink-400 mt-1">{t('chat.openInArtifact')}</div>
          </button>
        );
      })}
    </div>
  );
}
