import { useTranslation } from 'react-i18next';
import { Card } from './ui/Card';
import { Button } from './ui/Button';

interface Props {
  summary: { elapsed: string; tokens: number; cached: number; uncached: number };
  onOpenProject: () => void;
}

export default function DemoSummaryCard({ summary, onOpenProject }: Props) {
  const { t } = useTranslation();
  const cacheRate =
    summary.tokens > 0
      ? Math.round((summary.cached / (summary.cached + summary.uncached)) * 100)
      : null;
  return (
    <div className="h-full overflow-y-auto surface-paper p-8">
      <Card variant="paper" className="max-w-3xl mx-auto p-6">
        <div className="text-center mb-2">
          <div className="w-12 h-12 mx-auto mb-3 rounded-full bg-jade-light flex items-center justify-center">
            <svg
              className="w-6 h-6 text-jade"
              fill="none"
              viewBox="0 0 24 24"
              stroke="currentColor"
              strokeWidth={2.5}
            >
              <path strokeLinecap="round" strokeLinejoin="round" d="M4.5 12.75l6 6 9-13.5" />
            </svg>
          </div>
          <h2 className="font-display text-display font-bold text-ink-900">{t('demo.complete')}</h2>
          <p className="text-body text-ink-600 mt-1">{t('demo.summaryDesc')}</p>
        </div>
        <section className="mb-6">
          <h3 className="font-display text-ui text-ink-600 font-semibold title-rule-gold mb-3">
            {t('demo.summary')}
          </h3>
          <div className="grid grid-cols-1 sm:grid-cols-3 gap-3">
            <div className="bg-surface-parchment border border-fold rounded-lg p-3 text-center">
              <p className="text-caption text-ink-500 mb-1">{t('demo.duration')}</p>
              <p className="font-display text-xl text-ink-900 font-bold">{summary.elapsed}</p>
            </div>
            <div className="bg-surface-parchment border border-fold rounded-lg p-3 text-center">
              <p className="text-caption text-ink-500 mb-1">{t('demo.tokenConsumption')}</p>
              <p className="font-display text-xl text-ink-900 font-bold">
                {summary.tokens.toLocaleString()}
              </p>
              <p className="text-caption text-ink-500 mt-1">
                {t('demo.cacheSummary', {
                  cached: summary.cached.toLocaleString(),
                  uncached: summary.uncached.toLocaleString(),
                })}
              </p>
            </div>
            <div className="bg-surface-parchment border border-fold rounded-lg p-3 text-center">
              <p className="text-caption text-ink-500 mb-1">{t('demo.cacheHitRate')}</p>
              <p className="font-display text-xl text-ink-900 font-bold">
                {cacheRate !== null ? `${cacheRate}%` : 'N/A'}
              </p>
            </div>
          </div>
        </section>
        <section className="mb-6">
          <h3 className="font-display text-ui text-ink-600 font-semibold title-rule-gold mb-3">
            {t('demo.nextSteps')}
          </h3>
          <ul className="space-y-2 text-body text-ink-700">
            <li className="leading-relaxed">
              <strong>{t('demo.openRealProject')}</strong> — {t('demo.openRealProjectDesc')}
            </li>
            <li className="leading-relaxed">
              <strong>{t('demo.adjustParticipation')}</strong> — {t('demo.adjustParticipationDesc')}{' '}
              <code className="text-vermillion bg-vermillion-light px-1 rounded text-ui">
                /level-2
              </code>{' '}
              {t('demo.switchApprovalMode')}
            </li>
          </ul>
        </section>
        <div className="flex justify-center gap-3">
          <Button variant="secondary" onClick={onOpenProject}>
            {t('demo.openRealProject')}
          </Button>
        </div>
      </Card>
    </div>
  );
}
