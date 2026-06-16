import { useTranslation } from 'react-i18next';

export default function LangSwitcher() {
  const { i18n } = useTranslation();
  const current = i18n.language?.startsWith('en') ? 'en' : 'zh';

  const toggle = () => {
    const next = current === 'en' ? 'zh' : 'en';
    i18n.changeLanguage(next);
    localStorage.setItem('shuji_lang', next);
  };

  return (
    <button
      onClick={toggle}
      className="text-xs px-2 py-1 rounded border border-ink-700 text-ink-400 hover:text-ink-200 hover:border-ink-500 transition-colors"
      title={current === 'en' ? 'Switch to Chinese' : '切换到英文'}
    >
      {current === 'en' ? '中文' : 'EN'}
    </button>
  );
}
