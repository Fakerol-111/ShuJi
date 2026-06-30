import { useEffect, useCallback, useState } from 'react';
import { createPortal } from 'react-dom';
import { useTranslation } from 'react-i18next';
import { EFFORT_LABELS, ROLE_BUILTIN_EFFORT } from '../constants/reasoning';
import { deptKeyToRoleName } from '../constants';
import { setReasoningConfig as setReasoningConfigApi } from '../api';
import { useClickOutside } from '../hooks/useClickOutside';
import type { ReasoningConfig, ReasoningEffort, RoleReasoningConfig } from '../types';

const EFFORT_OPTIONS: ReasoningEffort[] = ['low', 'medium', 'high'];

interface ReasoningPopoverProps {
  roleKey: string;
  roleLabel: string;
  config: ReasoningConfig;
  onClose: () => void;
  anchorRect: DOMRect;
  onSaved?: (config: ReasoningConfig) => void;
}

export default function ReasoningPopover({
  roleKey,
  roleLabel,
  config,
  onClose,
  anchorRect,
  onSaved,
}: ReasoningPopoverProps) {
  const { i18n } = useTranslation();
  const lang = i18n.language?.startsWith('en') ? 'en' : 'zh';

  const roleName = deptKeyToRoleName(roleKey);
  const override = config.roles[roleName] as RoleReasoningConfig | undefined;
  const hasOverride = !!override;

  const effectiveEnabled = hasOverride ? (override!.enabled ?? config.enabled) : config.enabled;
  const effectiveEffort: ReasoningEffort =
    hasOverride && override!.effort
      ? override!.effort!
      : (ROLE_BUILTIN_EFFORT[roleLabel] ?? config.effort);

  const [saving, setSaving] = useState(false);
  const containerRef = useClickOutside(onClose);

  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      if (e.key === 'Escape') onClose();
    };
    document.addEventListener('keydown', handler);
    return () => document.removeEventListener('keydown', handler);
  }, [onClose]);

  const save = useCallback(
    async (roles: Record<string, RoleReasoningConfig>) => {
      setSaving(true);
      try {
        const next: ReasoningConfig = { ...config, roles };
        await setReasoningConfigApi(next);
        onSaved?.(next);
      } finally {
        setSaving(false);
      }
    },
    [config, onSaved]
  );

  const toggleEnabled = () => {
    const nextEnabled = !effectiveEnabled;
    save({
      ...config.roles,
      [roleName]: {
        ...override,
        enabled: nextEnabled,
        effort: override?.effort ?? effectiveEffort,
      },
    });
  };

  const setEffort = (e: ReasoningEffort) => {
    save({
      ...config.roles,
      [roleName]: {
        ...override,
        enabled: true,
        effort: e,
      },
    });
  };

  const style: React.CSSProperties = {
    position: 'fixed',
    top: Math.min(anchorRect.top, window.innerHeight - 200),
    left: anchorRect.right + 6,
  };

  return createPortal(
    <div ref={containerRef} style={style} className="z-[60] w-52">
      <div className="bg-surface-elevated border border-fold rounded-lg shadow-lg overflow-hidden">
        {/* Header */}
        <div className="px-3 py-2 border-b border-fold/50">
          <div className="flex items-center justify-between">
            <span className="text-ui font-semibold text-ink-800">{roleLabel}</span>
            <span className="text-caption text-ink-400">
              {lang === 'en' ? 'Reasoning' : '思考'}
            </span>
          </div>
        </div>

        {/* Enable/disable toggle */}
        <div className="px-3 py-2.5 border-b border-fold/50">
          <button
            type="button"
            onClick={toggleEnabled}
            disabled={saving}
            className="flex items-center justify-between w-full group"
          >
            <span className="text-caption text-ink-700 font-medium">
              {lang === 'en' ? 'Thinking mode' : '思考模式'}
            </span>
            <span
              className={`relative w-8 h-4.5 rounded-full transition-colors ${
                effectiveEnabled ? 'bg-gold' : 'bg-ink-300'
              }`}
            >
              <span
                className={`absolute top-0.5 left-0.5 w-3.5 h-3.5 rounded-full bg-white shadow-sm transition-transform ${
                  effectiveEnabled ? 'translate-x-3.5' : ''
                }`}
              />
            </span>
          </button>
        </div>

        {/* Effort chips — only when enabled */}
        {effectiveEnabled && (
          <div className="px-3 py-2.5 border-b border-fold/50">
            <div className="text-caption text-ink-500 mb-1.5">
              {lang === 'en' ? 'Intensity' : '推理强度'}
            </div>
            <div className="flex gap-1.5">
              {EFFORT_OPTIONS.map((e) => (
                <button
                  key={e}
                  type="button"
                  onClick={() => setEffort(e)}
                  disabled={saving}
                  className={`flex-1 px-1.5 py-1 rounded-md text-caption font-medium border transition-all ${
                    effectiveEffort === e
                      ? 'bg-gold/20 border-gold/50 text-ink-800'
                      : 'bg-transparent border-ink-200 text-ink-500 hover:border-ink-400'
                  }`}
                >
                  {EFFORT_LABELS[e][lang]}
                </button>
              ))}
            </div>
          </div>
        )}

        {/* Description */}
        <div className="px-3 py-2">
          <p className="text-caption text-ink-500 leading-relaxed">
            {effectiveEnabled
              ? EFFORT_LABELS[effectiveEffort][lang === 'en' ? 'descEn' : 'desc']
              : lang === 'en'
                ? 'Thinking disabled for this department'
                : '该部门已关闭思考模式'}
          </p>
        </div>
      </div>
    </div>,
    document.body
  );
}
