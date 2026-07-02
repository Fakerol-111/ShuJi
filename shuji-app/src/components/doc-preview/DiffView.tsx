import { useTranslation } from 'react-i18next';

export default function DiffView({ diff, audit = false }: { diff: string; audit?: boolean }) {
  const { t } = useTranslation();
  if (!diff) {
    return <div className="p-6 text-body text-ink-400 text-center">{t('docPreview.noDiff')}</div>;
  }
  const lines = diff.split('\n');

  return (
    <div
      className="doc-preview-diff min-w-0 rounded-lg border overflow-hidden"
      style={{ borderColor: 'var(--code-border)', backgroundColor: 'var(--code-bg)' }}
    >
      <div
        className="h-8 flex items-center px-3 text-[11px] font-mono shrink-0"
        style={{
          backgroundColor: 'var(--code-tab-bg)',
          borderBottom: '1px solid var(--code-border)',
          color: 'var(--code-muted)',
        }}
      >
        <span>{audit ? t('docPreview.auditDiffHeader') : 'Unified Diff'}</span>
      </div>
      <div className="doc-preview-diff-scroll overflow-auto min-w-0 text-[13px] leading-[22px] font-[Cascadia_Code,JetBrains_Mono,Consolas,Menlo,Monaco,monospace]">
        <table className="w-full border-separate border-spacing-0">
          <tbody>
            {lines.map((line, i) => {
              let bgColor = 'transparent';
              let textColor = 'var(--code-text)';
              if (line.startsWith('+') && !line.startsWith('+++')) {
                bgColor = 'rgba(34,197,94,0.10)';
                textColor = '#16a34a';
              } else if (line.startsWith('-') && !line.startsWith('---')) {
                bgColor = 'rgba(239,68,68,0.10)';
                textColor = '#dc2626';
              } else if (line.startsWith('@@')) {
                textColor = 'var(--code-line-num)';
              } else if (line.startsWith('---') || line.startsWith('+++')) {
                textColor = 'var(--code-line-num)';
              }
              return (
                <tr key={i} style={{ backgroundColor: bgColor }}>
                  <td className="pl-4 pr-6 whitespace-pre align-top" style={{ color: textColor }}>
                    {line || ' '}
                  </td>
                </tr>
              );
            })}
          </tbody>
        </table>
      </div>
    </div>
  );
}
