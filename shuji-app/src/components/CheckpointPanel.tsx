import { useState, useEffect, useCallback } from 'react';
import { useTranslation } from 'react-i18next';
import { listCheckpoints, restoreCheckpoint } from '../api';
import { DEPT_META_BY_KEY } from '../constants';
import { formatError } from '../utils/error';
import type { CheckpointEntry } from '../types';

interface RestoreConfirm {
  commit: string;
  desc: string;
}

function roleLabel(role: string, lang: string): string {
  const meta = DEPT_META_BY_KEY[role];
  if (meta) return lang === 'en' ? meta.labelEn : meta.label;
  return role;
}

function formatTime(ts: string, lang: string): string {
  try {
    const d = new Date(ts);
    const locale = lang === 'en' ? 'en-US' : 'zh-CN';
    return d.toLocaleString(locale, {
      month: '2-digit',
      day: '2-digit',
      hour: '2-digit',
      minute: '2-digit',
    });
  } catch {
    return ts;
  }
}

export default function CheckpointPanel() {
  const { t, i18n } = useTranslation();
  const lang = i18n.language?.startsWith('en') ? 'en' : 'zh';
  const [entries, setEntries] = useState<CheckpointEntry[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [restoring, setRestoring] = useState(false);
  const [restoreMsg, setRestoreMsg] = useState<string | null>(null);
  const [confirm, setConfirm] = useState<RestoreConfirm | null>(null);

  const fetch = useCallback(async () => {
    try {
      setLoading(true);
      const data = await listCheckpoints();
      setEntries(data);
      setError(null);
    } catch (e) {
      setError(formatError(e));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    fetch();
  }, [fetch]);

  const handleRestore = async () => {
    if (!confirm) return;
    setRestoring(true);
    setRestoreMsg(null);
    try {
      const msg = await restoreCheckpoint(confirm.commit);
      setRestoreMsg(msg);
      setConfirm(null);
    } catch (e) {
      setError(formatError(e));
    } finally {
      setRestoring(false);
    }
  };

  // ── Loading state ──
  if (loading && entries.length === 0) {
    return (
      <div className="flex items-center justify-center h-32 text-xs text-ink-400">{t('common.loading')}</div>
    );
  }

  // ── Error state ──
  if (error && entries.length === 0) {
    return (
      <div className="p-3">
        <div className="text-xs text-red-600 bg-red-50 border border-red-200 rounded p-2">
          {error}
        </div>
        <button onClick={fetch} className="mt-2 text-xs text-ink-500 hover:text-ink-700 underline">
          {t('common.retry')}
        </button>
      </div>
    );
  }

  return (
    <div className="flex flex-col h-full relative">
      {/* Header */}
      <div className="h-9 px-3 border-b border-ink-200 flex items-center justify-between bg-ink-50 shrink-0">
        <span className="text-xs font-semibold text-ink-700">{t('checkpoint.title')}</span>
        <button
          onClick={fetch}
          className="text-[11px] text-ink-400 hover:text-ink-600"
          title={t('common.refresh')}
        >
          {t('common.refresh')}
        </button>
      </div>

      {/* Restore success message */}
      {restoreMsg && (
        <div className="mx-3 mt-2 p-2 text-xs text-green-700 bg-green-50 border border-green-200 rounded">
          {restoreMsg}
        </div>
      )}

      {/* Body */}
      <div className="flex-1 min-h-0 overflow-y-auto p-3 space-y-2">
        {/* Empty state */}
        {entries.length === 0 && !loading && (
          <div className="text-xs text-ink-400 text-center py-8">{t('checkpoint.noCheckpoints')}</div>
        )}

        {/* Checkpoint list */}
        {entries.map((entry) => (
          <div
            key={entry.commit}
            className="border border-ink-200 rounded p-2.5 text-xs space-y-1 hover:border-ink-300 transition-colors"
          >
            <div className="flex items-center justify-between">
              <span className="font-medium text-ink-700">{roleLabel(entry.role, lang)}</span>
              <span className="text-[10px] text-ink-400">{formatTime(entry.ts, lang)}</span>
            </div>
            <div className="text-ink-600 truncate" title={entry.description}>
              {entry.description}
            </div>
            <div className="flex items-center justify-between">
              <code className="text-[10px] text-ink-400 font-mono">{entry.commit.slice(0, 7)}</code>
              <button
                onClick={() => setConfirm({ commit: entry.commit, desc: entry.description })}
                disabled={restoring}
                className="text-[10px] px-2 py-0.5 rounded border border-ink-300 text-ink-600 hover:bg-ink-100 hover:border-ink-400 disabled:opacity-40 transition-colors"
              >
                {t('common.restore')}
              </button>
            </div>
          </div>
        ))}
      </div>

      {/* Restore confirmation dialog */}
      {confirm && (
        <div className="absolute inset-0 bg-black/30 flex items-center justify-center z-50">
          <div className="bg-white rounded-lg shadow-xl mx-4 p-4 max-w-sm w-full">
            <h3 className="text-sm font-semibold text-ink-800 mb-2">{t('checkpoint.confirmRestore')}</h3>
            <p className="text-xs text-ink-600 mb-3 leading-relaxed">
              {t('checkpoint.restoreToCommit', {
                commit: confirm.commit.slice(0, 7),
                desc: confirm.desc,
              })}
            </p>
            <ul className="text-xs text-ink-500 space-y-1 mb-4 list-disc list-inside">
              <li>{t('checkpoint.restoreWarning1')}</li>
              <li>{t('checkpoint.restoreWarning2')}</li>
              <li>{t('checkpoint.restoreWarning3')}</li>
            </ul>
            <div className="flex justify-end gap-2">
              <button
                onClick={() => setConfirm(null)}
                disabled={restoring}
                className="px-3 py-1.5 text-xs rounded border border-ink-300 text-ink-600 hover:bg-ink-50 disabled:opacity-40"
              >
                {t('common.cancel')}
              </button>
              <button
                onClick={handleRestore}
                disabled={restoring}
                className="px-3 py-1.5 text-xs rounded bg-vermillion text-white hover:bg-vermillion-dark disabled:opacity-40"
              >
                {restoring ? t('checkpoint.restoring') : t('checkpoint.confirmRestore')}
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
