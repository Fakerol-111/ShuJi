import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import type { ExperienceLevel } from '../utils/uiPrefs';
import { clearProjectOnboardingDone } from '../utils/uiPrefs';
import { GlossaryTerm } from './GlossaryTerm';

interface DeptInfo {
  name: string;
  aliases: string[];
  role: string;
}

const DEPARTMENTS: DeptInfo[] = [
  { name: '内阁', aliases: ['neige'], role: 'helpDept.neige' },
  { name: '中书令', aliases: ['zhongshuling'], role: 'helpDept.zhongshuling' },
  { name: '门下侍中', aliases: ['menxiashizhong'], role: 'helpDept.menxiashizhong' },
  { name: '尚书令', aliases: ['shangshuling'], role: 'helpDept.shangshuling' },
  { name: '吏部尚书', aliases: ['libushangshu'], role: 'helpDept.libushangshu' },
  { name: '兵部尚书', aliases: ['bingbushangshu'], role: 'helpDept.bingbushangshu' },
  { name: '工部尚书', aliases: ['gongbushangshu'], role: 'helpDept.gongbushangshu' },
  { name: '刑部尚书', aliases: ['xingbushangshu'], role: 'helpDept.xingbushangshu' },
  { name: '礼部尚书', aliases: ['liburshangshu'], role: 'helpDept.liburshangshu' },
];

const WORKFLOW_STEPS = [
  { stepKey: 'helpWorkflow.step1', dept: '→ 内阁', descKey: 'helpWorkflow.desc1' },
  { stepKey: 'helpWorkflow.step2', dept: '→ 中书令', descKey: 'helpWorkflow.desc2' },
  { stepKey: 'helpWorkflow.step3', dept: '→ 门下侍中', descKey: 'helpWorkflow.desc3' },
  { stepKey: 'helpWorkflow.step4', dept: '→ 你', descKey: 'helpWorkflow.desc4' },
  { stepKey: 'helpWorkflow.step5', dept: '→ 尚书令 → 各部', descKey: 'helpWorkflow.desc5' },
  { stepKey: 'helpWorkflow.step6', dept: '', descKey: 'helpWorkflow.desc6' },
];

const GLOSSARY_TERMS = [
  'cabinet',
  'artifact',
  'approval',
  'workflowGraph',
  'edict',
  'dutyBar',
] as const;

interface HelpDrawerProps {
  experienceLevel: ExperienceLevel;
  onExperienceLevelChange: (level: ExperienceLevel) => void;
  onReplayOnboarding?: () => void;
}

export default function HelpDrawer({
  experienceLevel,
  onExperienceLevelChange,
  onReplayOnboarding,
}: HelpDrawerProps) {
  const { t } = useTranslation();
  const [open, setOpen] = useState(false);

  return (
    <>
      <button
        onClick={() => setOpen(!open)}
        className="text-xs px-2 py-1 text-ink-400 hover:text-ink-100 hover:bg-ink-800 rounded transition-colors"
        title={t('helpDrawer.title')}
      >
        ?
      </button>

      {open && (
        <>
          <div className="fixed inset-0 z-40 bg-ink-950/30" onClick={() => setOpen(false)} />
          <div className="fixed right-0 top-0 bottom-0 z-50 w-[360px] bg-surface-paper shadow-xl border-l border-fold overflow-y-auto">
            <div className="px-5 py-4">
              <div className="flex items-center justify-between mb-4">
                <h2 className="font-display text-title font-bold text-ink-900">
                  {t('helpDrawer.systemName')}
                </h2>
                <button
                  onClick={() => setOpen(false)}
                  className="text-ink-400 hover:text-ink-600 text-lg leading-none"
                >
                  &times;
                </button>
              </div>

              <div className="space-y-4">
                <Section title={t('helpDrawer.experience')}>
                  <div className="flex gap-2">
                    <ModeButton
                      active={experienceLevel === 'beginner'}
                      onClick={() => onExperienceLevelChange('beginner')}
                      label={t('helpDrawer.beginnerMode')}
                    />
                    <ModeButton
                      active={experienceLevel === 'advanced'}
                      onClick={() => onExperienceLevelChange('advanced')}
                      label={t('helpDrawer.advancedMode')}
                    />
                  </div>
                  <p className="text-caption text-ink-500 mt-2 leading-relaxed">
                    {experienceLevel === 'beginner'
                      ? t('helpDrawer.beginnerHint')
                      : t('helpDrawer.advancedHint')}
                  </p>
                  {onReplayOnboarding && (
                    <button
                      type="button"
                      onClick={() => {
                        clearProjectOnboardingDone();
                        onReplayOnboarding();
                        setOpen(false);
                      }}
                      className="mt-2 text-caption text-gold-700 hover:underline"
                    >
                      {t('helpDrawer.replayOnboarding')}
                    </button>
                  )}
                </Section>

                <Section title={t('helpDrawer.glossary')}>
                  <dl className="space-y-2 text-body">
                    {GLOSSARY_TERMS.map((term) => (
                      <div key={term}>
                        <dt className="font-medium text-ink-800">
                          <GlossaryTerm term={term}>{t(`glossary.${term}.label`)}</GlossaryTerm>
                        </dt>
                        <dd className="text-ink-600 text-caption leading-relaxed pl-0 mt-0.5">
                          {t(`glossary.${term}.hint`)}
                        </dd>
                      </div>
                    ))}
                  </dl>
                </Section>

                <Section title={t('helpDrawer.workflow')}>
                  <div className="space-y-2">
                    {WORKFLOW_STEPS.map((ws, i) => (
                      <div key={i} className="flex gap-2 text-body leading-relaxed">
                        <span className="text-ink-400 w-4 shrink-0">{i + 1}.</span>
                        <div className="min-w-0">
                          <span className="font-medium text-ink-800">{t(ws.stepKey)}</span>
                          {ws.dept && <span className="text-vermillion ml-1">{ws.dept}</span>}
                          <div className="text-ink-600">{t(ws.descKey)}</div>
                        </div>
                      </div>
                    ))}
                  </div>
                </Section>

                <Section title={t('helpDrawer.departments')}>
                  <div className="space-y-1">
                    {DEPARTMENTS.map((d) => (
                      <div key={d.name} className="flex gap-2 text-body py-1">
                        <span className="font-medium text-ink-800 w-16 shrink-0">{d.name}</span>
                        <span className="text-ink-600">{t(d.role)}</span>
                      </div>
                    ))}
                  </div>
                </Section>

                <Section title={t('helpDrawer.quickTips')}>
                  <ul className="text-body text-ink-600 space-y-1 list-disc pl-5 leading-relaxed">
                    <li>{t('helpDrawer.tip1')}</li>
                    <li>{t('helpDrawer.tip2')}</li>
                    <li>{t('helpDrawer.tip3')}</li>
                    <li>{t('helpDrawer.tip4')}</li>
                    <li>{t('helpDrawer.tip5')}</li>
                    <li>{t('helpDrawer.tip6')}</li>
                  </ul>
                </Section>
              </div>
            </div>
          </div>
        </>
      )}
    </>
  );
}

function ModeButton({
  active,
  onClick,
  label,
}: {
  active: boolean;
  onClick: () => void;
  label: string;
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      className={`flex-1 px-2 py-1.5 rounded-lg text-caption border transition-colors ${
        active
          ? 'border-gold/50 bg-gold/10 text-ink-900 font-medium'
          : 'border-fold text-ink-500 hover:bg-ink-100/50'
      }`}
    >
      {label}
    </button>
  );
}

function Section({ title, children }: { title: string; children: React.ReactNode }) {
  const [collapsed, setCollapsed] = useState(false);
  return (
    <div className="border border-fold rounded-lg overflow-hidden">
      <button
        type="button"
        onClick={() => setCollapsed(!collapsed)}
        className="title-rule-gold w-full flex items-center justify-between px-3 py-2 bg-surface-parchment text-ui font-semibold text-ink-700 hover:bg-ink-100 transition-colors"
      >
        {title}
        <span className="text-ink-400">{collapsed ? '+' : '-'}</span>
      </button>
      {!collapsed && <div className="p-3">{children}</div>}
    </div>
  );
}
