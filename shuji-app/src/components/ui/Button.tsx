import type { ButtonHTMLAttributes, ReactNode } from 'react';

type Variant = 'primary' | 'secondary' | 'ghost' | 'danger' | 'seal';

interface Props extends ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: Variant;
  children: ReactNode;
}

const variantStyles: Record<Variant, string> = {
  primary: 'bg-ink-900 text-ink-50 hover:bg-ink-800',
  secondary: 'border border-fold bg-surface-elevated text-ink-800 hover:bg-ink-100',
  ghost: 'text-ink-600 hover:bg-ink-100',
  danger: 'bg-vermillion text-white hover:bg-vermillion-dark',
  seal: 'border-2 border-vermillion text-vermillion hover:bg-vermillion-light',
};

export function Button({
  variant = 'primary',
  className = '',
  children,
  disabled,
  ...rest
}: Props) {
  return (
    <button
      className={`text-ui px-4 py-2 rounded-lg font-medium transition-colors
        ${variantStyles[variant]}
        ${disabled ? 'opacity-40 cursor-not-allowed' : ''}
        ${className}`}
      disabled={disabled}
      {...rest}
    >
      {children}
    </button>
  );
}
