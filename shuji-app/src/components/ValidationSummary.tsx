import { useTranslation } from 'react-i18next';
import type { ValidationReport } from '../types';

interface Props {
  report: ValidationReport | null;
  loading?: boolean;
}

export function ValidationSummary({ report, loading }: Props) {
  const { t } = useTranslation();

  if (loading) {
    return (
      <div className="validation-summary validation-summary--loading">
        <span className="validation-summary__icon">⏳</span>
        <span>{t('validation.validating')}</span>
      </div>
    );
  }

  if (!report) {
    return (
      <div className="validation-summary validation-summary--empty">
        <span className="validation-summary__icon">—</span>
        <span>{t('validation.noReport')}</span>
      </div>
    );
  }

  const passCount = report.checks.filter((c) => c.pass).length;
  const failCount = report.checks.filter((c) => !c.pass).length;

  return (
    <div
      className={`validation-summary ${report.overall_pass ? 'validation-summary--pass' : 'validation-summary--fail'}`}
    >
      <span className="validation-summary__icon">
        {report.overall_pass ? '✓' : '✗'}
      </span>
      <span className="validation-summary__status">
        {report.overall_pass ? t('validation.passed') : t('validation.failed')}
      </span>
      <span className="validation-summary__detail">
        {t('validation.passCount', { pass: passCount, total: report.checks.length })}
        {failCount > 0 && (
          <span className="validation-summary__fail-names">
            {' '}
            — {t('validation.failedItems')}: {report.checks.filter((c) => !c.pass).map((c) => c.name).join(', ')}
          </span>
        )}
      </span>
      <span className="validation-summary__project">{report.project_type}</span>
    </div>
  );
}
