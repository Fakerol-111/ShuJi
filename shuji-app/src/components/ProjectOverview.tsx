import { useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { listShujiTree, generateDeliveryReport } from '../api';
import type { ShujiEntry } from '../api';
import type { PlanInfo, Project, ValidationReport } from '../types';
import { getDeptMeta } from '../constants';
import { formatError } from '../utils/error';
import { ValidationSummary } from './ValidationSummary';
import { SealLogo } from './SealLogo';
import { Card } from './ui/Card';

const EDCT_EXAMPLES = ['分析代码库，了解项目结构', '实现用户注册功能', '修复登录页表单校验问题'];

interface ProjectOverviewProps {
  project: Project | null;
  activeDepts: string[];
  planInfo: PlanInfo | null;
  onOpenProject: () => void;
  onDocSelect?: (path: string) => void;
  onFillInput?: (text: string) => void;
  validationReport?: ValidationReport | null;
}

export default function ProjectOverview({
  project,
  activeDepts,
  planInfo,
  onOpenProject,
  onDocSelect,
  onFillInput,
  validationReport = null,
}: ProjectOverviewProps) {
  const { t } = useTranslation();
  const [latestDocs, setLatestDocs] = useState<ShujiEntry[]>([]);
  const [docsLoading, setDocsLoading] = useState(false);
  const [error, setError] = useState('');
  const [report, setReport] = useState<string | null>(null);
  const [reportLoading, setReportLoading] = useState(false);

  const handleGenerateReport = async () => {
    setReportLoading(true);
    try {
      const r = await generateDeliveryReport();
      setReport(r);
    } catch (e) {
      setError(formatError(e));
    } finally {
      setReportLoading(false);
    }
  };

  useEffect(() => {
    if (!project?.working_dir) return;
    setDocsLoading(true);
    setError('');
    listShujiTree(project.working_dir)
      .then((tree) => {
        setLatestDocs(
          flatten(tree)
            .filter((entry) => entry.path.startsWith('.shuji/') && entry.name.endsWith('.md'))
            .slice(0, 5)
        );
        setDocsLoading(false);
      })
      .catch((e) => {
        setError(formatError(e));
        setLatestDocs([]);
        setDocsLoading(false);
      });
  }, [project?.working_dir]);

  if (!project) {
    return (
      <div className="h-full flex items-center justify-center surface-paper">
        <div className="text-center max-w-md">
          <p className="text-ink-600 text-body mb-2">{t('projectOverview.noProject')}</p>
          <p className="text-ui text-ink-500 mb-6 leading-relaxed">{t('chat.idleDescription')}</p>
          <button
            onClick={onOpenProject}
            className="px-5 py-2 bg-ink-900 text-ink-50 text-ui rounded-lg hover:bg-ink-800 transition-colors"
          >
            {t('workspace.openProject')}
          </button>
        </div>
      </div>
    );
  }

  const done = planInfo?.batches.filter((b) => b.status === 'done').length || 0;
  const total = planInfo?.batches.length || 0;
  const isQuiet = activeDepts.length === 0;

  return (
    <div className="flex-1 overflow-y-auto surface-paper p-4 md:p-6">
      <Card variant="paper" className="max-w-3xl mx-auto p-5 md:p-6">
        <div className="flex items-start gap-3 mb-4">
          <SealLogo size={36} />
          <div className="min-w-0 flex-1">
            <div className="font-display text-display font-bold text-ink-900">{project.name}</div>
            <p className="text-caption text-ink-400 font-mono truncate">{project.working_dir}</p>
          </div>
        </div>

        {project.goal && (
          <p className="text-body text-ink-700 leading-relaxed mb-4 border-l-2 border-gold/40 pl-3">
            {project.goal}
          </p>
        )}

        {isQuiet && (
          <section className="mb-5">
            <h3 className="font-display text-ui text-ink-600 font-semibold title-rule-gold mb-2">
              {t('projectOverview.nextStep')}
            </h3>
            <p className="text-body text-ink-600 mb-2">{t('projectOverview.nextStepHint')}</p>
            <div className="flex flex-col gap-1.5">
              {EDCT_EXAMPLES.map((text) => (
                <button
                  key={text}
                  type="button"
                  onClick={() => onFillInput?.(text)}
                  className="flex items-center gap-2 px-3 py-2 rounded-lg border border-fold bg-surface-elevated hover:border-gold/40 text-left text-ui text-ink-700 transition-colors"
                >
                  <SealLogo size={14} />
                  {text}
                </button>
              ))}
            </div>
          </section>
        )}

        {validationReport && isQuiet && (
          <section className="mb-5">
            <h3 className="font-display text-ui text-ink-600 font-semibold title-rule-gold mb-2">
              {t('validation.latestReport')}
            </h3>
            <ValidationSummary report={validationReport} />
          </section>
        )}

        <section className="mb-5">
          <h3 className="font-display text-ui text-ink-600 font-semibold title-rule-gold mb-3">
            {t('projectOverview.activeDepts')}
          </h3>
          {activeDepts.length === 0 ? (
            <p className="text-body text-ink-400">{t('projectOverview.noActiveDepts')}</p>
          ) : (
            <div className="flex flex-wrap gap-2">
              {activeDepts.map((dept) => (
                <span
                  key={dept}
                  className="px-2 py-1 rounded-full bg-ink-100 text-ui text-ink-700 flex items-center gap-1"
                >
                  <span
                    className="animate-pulse"
                    style={{ color: getDeptMeta(dept)?.color || '#6b7280' }}
                  >
                    ●
                  </span>
                  {dept}
                </span>
              ))}
            </div>
          )}
        </section>

        <section className="mb-5">
          <h3 className="font-display text-ui text-ink-600 font-semibold title-rule-gold mb-3">
            {t('projectOverview.latestDocs')}
          </h3>
          {docsLoading ? (
            <p className="text-body text-ink-400">{t('common.loading')}</p>
          ) : error ? (
            <p className="text-body text-vermillion">{error}</p>
          ) : latestDocs.length === 0 ? (
            <p className="text-body text-ink-400">{t('projectOverview.noDocs')}</p>
          ) : (
            <div className="space-y-1">
              {latestDocs.map((doc) => (
                <div
                  key={doc.path}
                  onClick={() => onDocSelect?.(doc.path)}
                  className="flex items-center gap-2 text-body cursor-pointer hover:bg-ink-100/50 rounded px-1 -mx-1 transition-colors"
                >
                  <span className="font-mono text-ink-800">{doc.name.replace(/\.md$/, '')}</span>
                  <span className="text-ink-300">·</span>
                  <span className="text-ink-500">{doc.type_label}</span>
                </div>
              ))}
            </div>
          )}
        </section>

        {planInfo && total > 0 && (
          <section className="mb-5">
            <h3 className="font-display text-ui text-ink-600 font-semibold title-rule-gold mb-3">
              {t('projectOverview.gongbuPlan', { done, total })}
            </h3>
            <div className="w-full h-2 bg-ink-200 rounded-full overflow-hidden mb-2">
              <div
                className="h-full bg-gold"
                style={{ width: `${Math.round((done / total) * 100)}%` }}
              />
            </div>
            <div className="grid grid-cols-1 sm:grid-cols-2 gap-1">
              {planInfo.batches.map((b, i) => (
                <div
                  key={i}
                  className={`text-ui ${b.status === 'current' ? 'text-ink-900 font-medium' : 'text-ink-500'}`}
                >
                  · {b.name}
                </div>
              ))}
            </div>
          </section>
        )}

        <section className="pt-4 border-t border-fold">
          <button
            type="button"
            onClick={handleGenerateReport}
            disabled={reportLoading}
            className="px-4 py-2 bg-ink-900 text-ink-50 text-ui rounded-lg hover:bg-ink-800 transition-colors disabled:opacity-50"
          >
            {reportLoading ? t('common.loading') : t('projectOverview.generateReport')}
          </button>
          {report && (
            <div className="mt-3 p-3 rounded-lg bg-ink-100/50 border border-fold text-caption font-mono whitespace-pre-wrap text-ink-700 max-h-64 overflow-y-auto">
              {report}
            </div>
          )}
        </section>
      </Card>
    </div>
  );
}

function flatten(entries: ShujiEntry[]): ShujiEntry[] {
  return entries.flatMap((entry) => (entry.is_dir ? flatten(entry.children) : [entry]));
}
