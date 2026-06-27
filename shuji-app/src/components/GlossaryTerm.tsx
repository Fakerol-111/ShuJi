import type { ReactNode } from 'react';
import { useTranslation } from 'react-i18next';

interface GlossaryTermProps {
  term: string;
  children: ReactNode;
  className?: string;
}

/** Hover tooltip for product terms (架阁、朱批, etc.). */
export function GlossaryTerm({ term, children, className = '' }: GlossaryTermProps) {
  const { t } = useTranslation();
  const hint = t(`glossary.${term}.hint`, { defaultValue: '' });
  if (!hint) {
    return <span className={className}>{children}</span>;
  }
  return (
    <abbr
      title={hint}
      className={`no-underline border-b border-dotted border-ink-400/70 cursor-help ${className}`}
    >
      {children}
    </abbr>
  );
}
