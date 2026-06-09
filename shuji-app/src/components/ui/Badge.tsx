import type { ReactNode } from 'react';

interface Props {
  children: ReactNode;
  className?: string;
  color?: string;
}

export function Badge({ children, className = '', color }: Props) {
  return (
    <span
      className={`inline-flex items-center text-caption px-2 py-0.5 rounded-md font-medium
        ${color ? '' : 'bg-ink-100 text-ink-700'}
        ${className}`}
      style={color ? { backgroundColor: `${color}18`, color } : undefined}
    >
      {children}
    </span>
  );
}
