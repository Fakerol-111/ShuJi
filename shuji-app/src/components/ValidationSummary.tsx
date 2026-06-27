import { useTranslation } from 'react-i18next';
import type { ValidationReport } from '../types';

interface Props {
  report: ValidationReport | null;
  loading?: boolean;
  compact?: boolean;
}

export function ValidationSummary({ report, loading, compact = false }: Props) {
  const { t } = useTranslation();

  if (loading) {
    return (
      <div
        className={`validation-summary validation-summary--loading ${compact ? 'text-caption' : ''}`}
      >
        <span className="validation-summary__icon">⏳</span>
        <span>{t('validation.validating')}</span>
      </div>
    );
  }

  if (!report) {
    if (compact) return null;
    return (
      <div className="validation-summary validation-summary--empty">
        <span className="validation-summary__icon">—</span>
        <span>{t('validation.noReport')}</span>
      </div>
    );
  }

  const passCount = report.checks.filter((c) => c.pass).length;
  const failCount = report.checks.filter((c) => !c.pass).length;

  if (compact) {
    return (
      <span
        className={`inline-flex items-center gap-1 text-caption font-mono ${
          report.overall_pass ? 'text-jade' : 'text-vermillion'
        }`}
        title={t('validation.passCount', { pass: passCount, total: report.checks.length })}
      >
        {report.overall_pass ? '✓' : '✗'}{' '}
        {t('validation.passCount', { pass: passCount, total: report.checks.length })}
      </span>
    );
  }

  return (
    <div
      className={`flex flex-wrap items-center gap-x-2 gap-y-1 rounded-lg border px-2 py-1.5 text-caption ${
        report.overall_pass
          ? 'border-jade/30 bg-jade/5 text-jade-800'
          : 'border-vermillion/30 bg-vermillion/5 text-vermillion-dark'
      }`}
    >
      <span className="font-semibold">{report.overall_pass ? '✓' : '✗'}</span>
      <span className="font-medium">
        {report.overall_pass ? t('validation.passed') : t('validation.failed')}
      </span>
      <span className="text-ink-600">
        {t('validation.passCount', { pass: passCount, total: report.checks.length })}
        {failCount > 0 && (
          <span className="text-vermillion">
            {' '}
            — {t('validation.failedItems')}:{' '}
            {report.checks
              .filter((c) => !c.pass)
              .map((c) => c.name)
              .join(', ')}
          </span>
        )}
      </span>
      <span className="text-ink-400 font-mono">{report.project_type}</span>
    </div>
  );
}
