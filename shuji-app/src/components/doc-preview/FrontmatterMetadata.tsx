import { useTranslation } from 'react-i18next';

export default function FrontmatterMetadata({ meta }: { meta: Record<string, string> }) {
  const { t } = useTranslation();
  const labels: Record<string, string> = {
    id: 'ID',
    type: t('document.type'),
    author: t('document.author'),
    timestamp: t('document.time'),
    refs: t('document.refs'),
    status: t('document.status'),
  };
  const summaryParts = ['id', 'type', 'status'].map((key) => meta[key]).filter(Boolean);

  return (
    <details className="doc-preview-metadata mb-4 border-b border-fold pb-3 min-w-0">
      <summary className="text-caption font-mono text-ink-500 cursor-pointer select-none list-none [&::-webkit-details-marker]:hidden">
        <span className="text-ink-400">{t('docPreview.metadata')}</span>
        {summaryParts.length > 0 && (
          <span className="ml-2 text-ink-600">{summaryParts.join(' · ')}</span>
        )}
      </summary>
      <dl className="mt-2 grid grid-cols-1 sm:grid-cols-2 gap-x-4 gap-y-1 text-ui font-mono">
        {Object.entries(meta).map(([key, value]) => {
          const statusColor =
            key === 'status' && value === 'in_review'
              ? 'text-vermillion font-bold'
              : key === 'status' && value === 'approved'
                ? 'text-jade font-bold'
                : key === 'status' && value === 'rejected'
                  ? 'text-vermillion/60 font-bold'
                  : 'text-ink-700';
          if (key === 'notes' && !value) return null;
          if (key === 'status' && !value) return null;
          return (
            <div key={key} className="flex min-w-0 gap-2">
              <dt className="w-16 shrink-0 text-ink-400">{labels[key] || key}</dt>
              <dd className={`break-all min-w-0 ${statusColor}`}>{value}</dd>
            </div>
          );
        })}
      </dl>
    </details>
  );
}
