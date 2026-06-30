import '@testing-library/jest-dom';
import i18n from 'i18next';
import { initReactI18next } from 'react-i18next';
import en from '../i18n/locales/en.json';
import zh from '../i18n/locales/zh.json';

// Initialize i18n for tests without LanguageDetector (needs browser)
i18n.use(initReactI18next).init({
  resources: { en: { translation: en }, zh: { translation: zh } },
  lng: 'zh',
  fallbackLng: 'zh',
  interpolation: { escapeValue: false },
});

// jsdom lacks ResizeObserver — provide a minimal stub used by DashboardLayout
if (typeof globalThis.ResizeObserver === 'undefined') {
  globalThis.ResizeObserver = class {
    constructor(_cb: ResizeObserverCallback) {}
    observe() {}
    unobserve() {}
    disconnect() {}
  };
}
