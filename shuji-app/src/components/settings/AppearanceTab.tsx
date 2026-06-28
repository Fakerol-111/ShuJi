import { useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import {
  CODE_THEMES,
  FONT_SIZE_TIERS,
  getCodeTheme,
  setCodeTheme as persistCodeTheme,
  getFontSize,
  setFontSize as persistFontSize,
} from '../../constants';
import { SettingsSection, SettingsChip, SettingsHint } from './SettingsPrimitives';

export default function AppearanceTab() {
  const { t, i18n } = useTranslation();
  const lang = i18n.language?.startsWith('en') ? 'en' : 'zh';
  const [fontSize, setFontSize] = useState(getFontSize);
  const [codeTheme, setCodeTheme] = useState(getCodeTheme);

  const setFontSizeLocal = (key: string) => {
    persistFontSize(key);
    setFontSize(key);
    document.documentElement.dataset.fontSize = key;
  };

  const setCodeThemeLocal = (key: string) => {
    persistCodeTheme(key);
    setCodeTheme(key);
    document.documentElement.dataset.codeTheme = key;
  };

  useEffect(() => {
    document.documentElement.dataset.fontSize = fontSize;
  }, [fontSize]);

  useEffect(() => {
    document.documentElement.dataset.codeTheme = codeTheme;
  }, [codeTheme]);

  return (
    <div className="space-y-6">
      <SettingsSection title={t('settings.fontSize')} description={t('common.save')}>
        <div className="flex gap-2 flex-wrap">
          {Object.entries(FONT_SIZE_TIERS).map(([key, tier]) => (
            <SettingsChip
              key={key}
              selected={fontSize === key}
              onClick={() => setFontSizeLocal(key)}
              title={tier.description}
            >
              {lang === 'en' ? tier.labelEn : tier.label}
            </SettingsChip>
          ))}
        </div>
        <SettingsHint>
          {lang === 'en'
            ? FONT_SIZE_TIERS[fontSize as keyof typeof FONT_SIZE_TIERS]?.descriptionEn
            : FONT_SIZE_TIERS[fontSize as keyof typeof FONT_SIZE_TIERS]?.description}
        </SettingsHint>
      </SettingsSection>

      <SettingsSection title={t('settings.codeTheme')} description={t('common.save')} divider>
        <div className="flex gap-2 flex-wrap">
          {Object.entries(CODE_THEMES).map(([key, theme]) => (
            <SettingsChip
              key={key}
              selected={codeTheme === key}
              onClick={() => setCodeThemeLocal(key)}
            >
              {theme.label}
            </SettingsChip>
          ))}
        </div>
      </SettingsSection>

      <SettingsSection title={t('settings.about')} divider>
        <div className="text-sm text-ink-700 space-y-2 leading-relaxed">
          <p>
            <span className="font-medium text-ink-900">{t('settings.about')}</span>{' '}
            {t('settings.version')} — Preview
          </p>
          <p>{t('settings.aboutDescription')}</p>
        </div>
      </SettingsSection>
    </div>
  );
}
