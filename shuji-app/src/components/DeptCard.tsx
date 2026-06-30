import { useState, useRef } from 'react';
import { useTranslation } from 'react-i18next';
import DeptGlyph from './DeptGlyph';
import ReasoningPopover from './ReasoningPopover';
import { getDeptDisplayLabel, type DeptMeta } from '../constants';
import type { PlanInfo, ReasoningConfig } from '../types';

interface DeptCardProps {
  meta: DeptMeta;
  isActive: boolean;
  isSelected: boolean;
  hasError: boolean;
  latestAction: string;
  intent?: string;
  latestArtifact?: string | null;
  planInfo?: PlanInfo | null;
  reasoningConfig?: ReasoningConfig | null;
  onReasoningChange?: (config: ReasoningConfig) => void;
  onClick: () => void;
}

export default function DeptCard({
  meta,
  isActive,
  isSelected,
  hasError,
  latestAction,
  intent,
  latestArtifact,
  planInfo,
  reasoningConfig,
  onReasoningChange,
  onClick,
}: DeptCardProps) {
  const { t, i18n } = useTranslation();
  const lang = i18n.language?.startsWith('en') ? 'en' : 'zh';
  const displayLabel = getDeptDisplayLabel(meta, lang);
  const showPlan = meta.key === 'gongbushangshu' && planInfo && planInfo.batches.length > 0;
  const progress =
    showPlan && planInfo
      ? Math.round(
          (planInfo.batches.filter((b) => b.status === 'done').length / planInfo.batches.length) *
            100
        )
      : 0;

  const [showPopover, setShowPopover] = useState(false);
  const [anchorRect, setAnchorRect] = useState<DOMRect | null>(null);
  const brainRef = useRef<HTMLSpanElement>(null);

  const bgClass = isSelected
    ? `${meta.tintClass} ring-1 ring-offset-0`
    : isActive
      ? meta.tintActiveClass
      : 'bg-surface-elevated/60';

  return (
    <>
      <button
        onClick={onClick}
        aria-selected={isSelected}
        role="tab"
        className={`w-[calc(100%-1rem)] mx-2 mb-1.5 rounded-xl border border-fold/80 transition-all duration-200 overflow-hidden ${bgClass}`}
        style={{ borderLeftWidth: 3, borderLeftColor: meta.color }}
      >
        <div className="px-3 py-2.5">
          <div className="flex items-center gap-2">
            <DeptGlyph deptKey={meta.key} size={16} stroke={meta.color} />
            <span className="text-ui font-display font-semibold text-ink-800 truncate">
              {displayLabel}
            </span>
            {isActive && (
              <span className="w-1.5 h-1.5 rounded-full bg-gold animate-pulse shrink-0 ml-auto" />
            )}
            {hasError && (
              <span
                className="w-1.5 h-1.5 rounded-full bg-vermillion shrink-0"
                title={t('common.error')}
              />
            )}
            <span
              ref={brainRef}
              role="button"
              tabIndex={0}
              onClick={(e) => {
                e.stopPropagation();
                const rect = brainRef.current?.getBoundingClientRect();
                if (rect) {
                  setAnchorRect(rect);
                  setShowPopover((v) => !v);
                }
              }}
              className="shrink-0 p-0.5 rounded text-ink-400 hover:text-ink-700 hover:bg-ink-100/50 transition-colors ml-auto cursor-pointer"
              title={lang === 'en' ? 'Reasoning settings' : '思考设置'}
            >
              <svg
                width="12"
                height="12"
                viewBox="0 0 24 24"
                fill="none"
                stroke="currentColor"
                strokeWidth="2"
                strokeLinecap="round"
                strokeLinejoin="round"
              >
                <path d="M12 2a4 4 0 0 1 4 4c0 1.2-.6 2.3-1.5 3l-.5.5c.3.3.5.7.5 1.2 0 1-.8 1.8-1.8 1.8h-1.4c-1 0-1.8-.8-1.8-1.8 0-.5.2-.9.5-1.2l-.5-.5A4 4 0 0 1 12 2z" />
                <path d="M9 18h6" />
                <path d="M10 22h4" />
              </svg>
            </span>
          </div>
          {isActive && intent && (
            <div className="text-[10px] text-ink-400 uppercase tracking-wide mt-0.5">{intent}</div>
          )}
          {showPlan && (
            <div className="mt-1.5 flex items-center gap-1.5">
              <div className="flex-1 h-1 bg-ink-200 rounded-full overflow-hidden">
                <div
                  className="h-full rounded-full transition-all duration-500"
                  style={{ width: `${progress}%`, backgroundColor: meta.color }}
                />
              </div>
              <span className="text-caption text-ink-400 font-mono shrink-0">
                {planInfo!.batches.filter((b) => b.status === 'done').length}/
                {planInfo!.batches.length}
              </span>
            </div>
          )}
          {latestAction && !showPlan && (
            <div className="mt-0.5 text-caption text-ink-600 truncate leading-tight">
              {latestAction}
              {latestArtifact && (
                <span className="ml-1 font-mono text-ink-400">{latestArtifact}</span>
              )}
            </div>
          )}
        </div>
      </button>
      {showPopover && reasoningConfig && onReasoningChange && anchorRect && (
        <ReasoningPopover
          roleKey={meta.key}
          roleLabel={meta.label}
          config={reasoningConfig}
          onClose={() => setShowPopover(false)}
          anchorRect={anchorRect}
          onSaved={onReasoningChange}
        />
      )}
    </>
  );
}
