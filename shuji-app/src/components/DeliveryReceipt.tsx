import { useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { listCheckpoints } from '../api';
import { docIdToPath } from '../utils/docPath';
import { ValidationSummary } from './ValidationSummary';
import type { CheckpointEntry, RoundMetrics, ValidationReport } from '../types';

export interface DeliveryReceiptProps {
  validationReport: ValidationReport | null;
  validationLoading?: boolean;
  roundMetrics?: RoundMetrics | null;
  recentDocIds?: string[];
  activeDeptsCount?: number;
  onSelectDoc?: (docPath: string) => void;
  onRefreshValidation?: () => void;
}

function formatElapsed(startedAt: number): string {
  if (startedAt <= 0) return '';
  const secs = Math.floor((Date.now() - startedAt) / 1000);
  if (secs < 60) return `${secs}s`;
  if (secs < 3600) return `${Math.floor(secs / 60)}m${secs % 60}s`;
  return `${Math.floor(secs / 3600)}h${Math.floor((secs % 3600) / 60)}m`;
}

export default function DeliveryReceipt({
  validationReport,
  validationLoading = false,
  roundMetrics = null,
  recentDocIds = [],
  activeDeptsCount = 0,
  onSelectDoc,
  onRefreshValidation,
}: DeliveryReceiptProps) {
  const { t } = useTranslation();
  const [checkpoint, setCheckpoint] = useState<CheckpointEntry | null>(null);

  useEffect(() => {
    listCheckpoints(undefined, 1)
      .then((entries) => setCheckpoint(entries[0] ?? null))
      .catch(() => setCheckpoint(null));
  }, [validationReport?.ts, activeDeptsCount]);

  const showContent = !validationLoading && validationReport != null && activeDeptsCount === 0;

  if (!validationLoading && !validationReport) return null;
  if (!validationLoading && activeDeptsCount > 0) return null;

  const failedChecks = validationReport?.checks.filter((c) => !c.pass) ?? [];
  const tokenTotal = roundMetrics?.total_tokens ?? 0;
  const cached = roundMetrics?.cached_prompt_tokens ?? 0;
  const cachePct =
    roundMetrics && roundMetrics.prompt_tokens > 0
      ? Math.round((cached / roundMetrics.prompt_tokens) * 100)
      : null;

  return (
    <div className="rounded-xl border border-fold bg-surface-parchment/80 px-3 py-2.5 space-y-2">
      <div className="flex items-center justify-between gap-2">
        <span className="text-caption font-display font-semibold text-ink-700">
          {t('deliveryReceipt.title')}
        </span>
        {onRefreshValidation && (
          <button
            type="button"
            onClick={onRefreshValidation}
            className="text-caption text-gold hover:text-gold-dark"
          >
            {t('common.refresh')}
          </button>
        )}
      </div>

      <ValidationSummary report={validationReport} loading={validationLoading} />

      {!validationLoading && showContent && validationReport && (
        <>
          {recentDocIds.length > 0 && (
            <div>
              <span className="text-caption text-ink-500 font-medium block mb-1">
                {t('deliveryReceipt.artifacts')}
              </span>
              <div className="flex flex-wrap gap-1">
                {recentDocIds.slice(-6).map((docId) => (
                  <button
                    key={docId}
                    type="button"
                    onClick={() => onSelectDoc?.(docIdToPath(docId))}
                    className="px-2 py-0.5 rounded border border-ink-200 bg-surface-elevated font-mono text-caption text-ink-700 hover:border-gold/40"
                  >
                    {docId}
                  </button>
                ))}
              </div>
            </div>
          )}

          {roundMetrics && tokenTotal > 0 && (
            <div className="text-caption text-ink-600 flex flex-wrap gap-x-3 gap-y-0.5">
              <span>
                {t('deliveryReceipt.tokens')}:{' '}
                <span className="font-mono tabular-nums">{tokenTotal.toLocaleString()}</span>
              </span>
              {cachePct != null && cachePct > 0 && (
                <span>
                  {t('deliveryReceipt.cacheHit')}:{' '}
                  <span className="font-mono tabular-nums">{cachePct}%</span>
                </span>
              )}
              {roundMetrics.started_at > 0 && (
                <span>
                  {t('deliveryReceipt.duration')}:{' '}
                  <span className="font-mono">{formatElapsed(roundMetrics.started_at)}</span>
                </span>
              )}
            </div>
          )}

          {checkpoint && (
            <p className="text-caption text-ink-500">
              {t('deliveryReceipt.checkpoint')}:{' '}
              <span className="font-mono">{checkpoint.commit.slice(0, 8)}</span>
              {checkpoint.description && (
                <span className="text-ink-400"> — {checkpoint.description}</span>
              )}
            </p>
          )}

          {validationReport.overall_pass ? (
            <p className="text-caption text-jade">{t('deliveryReceipt.passHint')}</p>
          ) : (
            <div className="text-caption text-vermillion space-y-1">
              <p>{t('deliveryReceipt.failHint')}</p>
              {failedChecks.length > 0 && (
                <ul className="list-disc list-inside text-ink-600">
                  {failedChecks.map((c) => (
                    <li key={c.name}>
                      {c.name}: {c.summary}
                    </li>
                  ))}
                </ul>
              )}
            </div>
          )}
        </>
      )}
    </div>
  );
}
