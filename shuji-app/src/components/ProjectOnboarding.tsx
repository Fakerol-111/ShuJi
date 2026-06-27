import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Button } from './ui/Button';
import { markProjectOnboardingDone } from '../utils/uiPrefs';

const STEP_KEYS = ['onboarding.step1', 'onboarding.step2', 'onboarding.step3'] as const;

interface ProjectOnboardingProps {
  onClose: () => void;
}

export default function ProjectOnboarding({ onClose }: ProjectOnboardingProps) {
  const { t } = useTranslation();
  const [step, setStep] = useState(0);
  const isLast = step === STEP_KEYS.length - 1;

  const finish = () => {
    markProjectOnboardingDone();
    onClose();
  };

  return (
    <div className="fixed inset-0 z-[60] flex items-center justify-center">
      <div className="absolute inset-0 bg-ink-950/50" onClick={finish} aria-hidden />
      <div
        className="relative bg-surface-elevated border border-fold rounded-2xl shadow-2xl w-full max-w-lg mx-4 p-6"
        role="dialog"
        aria-labelledby="onboarding-title"
      >
        <div className="flex items-center justify-center gap-1.5 mb-4">
          {STEP_KEYS.map((_, i) => (
            <div
              key={i}
              className={`h-1.5 rounded-full transition-all ${
                i === step ? 'w-6 bg-gold' : 'w-1.5 bg-ink-300'
              }`}
            />
          ))}
        </div>

        <h2 id="onboarding-title" className="font-display text-title font-bold text-ink-900 mb-2">
          {t(`${STEP_KEYS[step]}.title`)}
        </h2>
        <p className="text-body text-ink-600 leading-relaxed mb-1">
          {t(`${STEP_KEYS[step]}.body`)}
        </p>
        <p className="text-caption text-ink-500 mb-6">{t(`${STEP_KEYS[step]}.hint`)}</p>

        <div className="flex items-center justify-between gap-3">
          <button
            type="button"
            onClick={finish}
            className="text-caption text-ink-500 hover:text-ink-700"
          >
            {t('onboarding.skip')}
          </button>
          <div className="flex gap-2">
            {step > 0 && (
              <Button variant="ghost" className="text-sm" onClick={() => setStep((s) => s - 1)}>
                {t('onboarding.back')}
              </Button>
            )}
            {isLast ? (
              <Button variant="seal" className="text-sm" onClick={finish}>
                {t('onboarding.start')}
              </Button>
            ) : (
              <Button variant="seal" className="text-sm" onClick={() => setStep((s) => s + 1)}>
                {t('onboarding.next')}
              </Button>
            )}
          </div>
        </div>
      </div>
    </div>
  );
}
