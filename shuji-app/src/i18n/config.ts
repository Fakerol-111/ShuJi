import i18n from 'i18next';
import { initReactI18next } from 'react-i18next';
import LanguageDetector from 'i18next-browser-languagedetector';

import en from './locales/en.json';
import zh from './locales/zh.json';

const SUPPORTED_LANGUAGES = ['en', 'zh'] as const;
export type SupportedLang = (typeof SUPPORTED_LANGUAGES)[number];

export function isSupportedLang(s: string): s is SupportedLang {
  return SUPPORTED_LANGUAGES.includes(s as SupportedLang);
}

i18n
  .use(LanguageDetector)
  .use(initReactI18next)
  .init({
    resources: {
      en: { translation: en },
      zh: { translation: zh },
    },
    fallbackLng: 'zh',
    detection: {
      order: ['localStorage', 'navigator'],
      caches: ['localStorage'],
      lookupLocalStorage: 'shuji_lang',
    },
    interpolation: {
      escapeValue: false,
    },
  });

export default i18n;
