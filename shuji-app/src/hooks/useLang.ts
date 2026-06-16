import { useTranslation } from 'react-i18next';

/** Returns current UI language: 'en' or 'zh'. Syncs with i18n. */
export function useLang(): 'en' | 'zh' {
  const { i18n } = useTranslation();
  return i18n.language?.startsWith('en') ? 'en' : 'zh';
}
