import type { ReportTabProps } from './types';

export default function ReportTab({ t, report, reportLoading, onLoadReport }: ReportTabProps) {
  return (
    <div className="p-3 flex-1 overflow-y-auto min-h-0">
      {!report && !reportLoading && (
        <button
          onClick={onLoadReport}
          className="px-3 py-1.5 rounded bg-ink-700 text-white text-caption hover:bg-ink-600"
        >
          {t('audit.generateReport')}
        </button>
      )}
      {reportLoading && <div className="text-caption text-ink-400">{t('common.loading')}</div>}
      {report && (
        <div className="text-caption text-ink-700 whitespace-pre-wrap font-mono text-[11px] leading-relaxed">
          {report}
        </div>
      )}
    </div>
  );
}
