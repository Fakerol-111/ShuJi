import type { ValidationReport } from '../types';

interface Props {
  report: ValidationReport | null;
  loading?: boolean;
}

export function ValidationSummary({ report, loading }: Props) {
  if (loading) {
    return (
      <div className="validation-summary validation-summary--loading">
        <span className="validation-summary__icon">⏳</span>
        <span>验证加载中…</span>
      </div>
    );
  }

  if (!report) {
    return (
      <div className="validation-summary validation-summary--empty">
        <span className="validation-summary__icon">—</span>
        <span>暂无验证报告</span>
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
        {report.overall_pass ? '验证通过' : '验证未通过'}
      </span>
      <span className="validation-summary__detail">
        {passCount}/{report.checks.length} 项通过
        {failCount > 0 && (
          <span className="validation-summary__fail-names">
            {' '}
            — 失败: {report.checks.filter((c) => !c.pass).map((c) => c.name).join(', ')}
          </span>
        )}
      </span>
      <span className="validation-summary__project">{report.project_type}</span>
    </div>
  );
}
