import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Button } from './ui/Button';

const STORAGE_KEY = 'shuji_demo_tour_done';

const MOCK_STEPS = [
  {
    titleKey: 'demo.tourMockTitle1',
    descKey: 'demo.tourMockDesc1',
  },
  {
    titleKey: 'demo.tourMockTitle2',
    descKey: 'demo.tourMockDesc2',
  },
  {
    titleKey: 'demo.tourMockTitle3',
    descKey: 'demo.tourMockDesc3',
  },
  {
    titleKey: 'demo.tourMockTitle4',
    descKey: 'demo.tourMockDesc4',
  },
];

const DEMO_STEPS = [
  {
    titleKey: 'demo.tourTitle1',
    descKey: 'demo.tourDesc1',
  },
  {
    titleKey: 'demo.tourTitle2',
    descKey: 'demo.tourDesc2',
  },
  {
    titleKey: 'demo.tourTitle3',
    descKey: 'demo.tourDesc3',
  },
  {
    titleKey: 'demo.tourTitle4',
    descKey: 'demo.tourDesc4',
  },
];

interface DemoTourProps {
  onClose: () => void;
  mockMode?: boolean;
}

export default function DemoTour({ onClose, mockMode }: DemoTourProps & { mockMode?: boolean }) {
  const { t } = useTranslation();
  const STEPS = mockMode ? MOCK_STEPS : DEMO_STEPS;
  const [step, setStep] = useState(0);
  const isLast = step === STEPS.length - 1;

  const handleDone = () => {
    try {
      localStorage.setItem(STORAGE_KEY, 'true');
    } catch {
      /* localStorage unavailable */
    }
    onClose();
  };

  const handleSkip = () => {
    handleDone();
  };

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center">
      {/* Backdrop */}
      <div className="absolute inset-0 bg-ink-950/60" onClick={handleSkip} />

      {/* Card */}
      <div className="relative bg-surface-elevated border border-fold rounded-2xl shadow-2xl w-full max-w-md mx-4 p-6">
        {/* Step indicator dots */}
        <div className="flex items-center justify-center gap-1.5 mb-4">
          {STEPS.map((_, i) => (
            <div
              key={i}
              className={`w-2 h-2 rounded-full transition-colors ${
                i === step ? 'bg-vermillion' : 'bg-ink-300'
              }`}
            />
          ))}
        </div>

        {/* Step number */}
        <p className="text-caption text-ink-500 font-medium text-center mb-1">
          {step + 1} / {STEPS.length}
        </p>

        {/* Title */}
        <h2 className="font-display text-display font-bold text-ink-900 text-center mb-2">
          {t(STEPS[step].titleKey)}
        </h2>

        {/* Description */}
        <p className="text-body text-ink-700 text-center leading-relaxed mb-6">
          {t(STEPS[step].descKey)}
        </p>

        {/* Actions */}
        <div className="flex items-center justify-between">
          <button
            onClick={handleSkip}
            className="text-ui text-ink-500 hover:text-ink-700 px-2 py-1 transition-colors"
          >
            {t('demo.skipTour')}
          </button>

          <div className="flex items-center gap-2">
            {step > 0 && (
              <Button variant="secondary" onClick={() => setStep(step - 1)}>
                {t('common.back')}
              </Button>
            )}
            {isLast ? (
              <Button variant="primary" onClick={handleDone}>
                {t('demo.gotIt')}
              </Button>
            ) : (
              <Button variant="primary" onClick={() => setStep(step + 1)}>
                {t('common.next')}
              </Button>
            )}
          </div>
        </div>
      </div>
    </div>
  );
}
