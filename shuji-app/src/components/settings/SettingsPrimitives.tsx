import type { ReactNode, InputHTMLAttributes, ButtonHTMLAttributes } from 'react';
import { Input } from '../ui/Input';
import { Button } from '../ui/Button';

/* ── Section ─────────────────────────────────────────── */

interface SettingsSectionProps {
  title: string;
  description?: string;
  children: ReactNode;
  className?: string;
  divider?: boolean;
}

export function SettingsSection({
  title,
  description,
  children,
  className = '',
  divider,
}: SettingsSectionProps) {
  return (
    <section
      className={`space-y-3 ${divider ? 'pt-6 border-t border-border-fold' : ''} ${className}`}
    >
      <div>
        <h3 className="text-sm font-semibold text-ink-900">{title}</h3>
        {description && (
          <p className="mt-1 text-xs text-ink-600 leading-relaxed">{description}</p>
        )}
      </div>
      {children}
    </section>
  );
}

/* ── Chips ─────────────────────────────────────────── */

interface SettingsChipProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  selected?: boolean;
  children: ReactNode;
  size?: 'sm' | 'md';
}

export function SettingsChip({
  selected,
  children,
  size = 'md',
  className = '',
  ...rest
}: SettingsChipProps) {
  const sizeClass =
    size === 'sm' ? 'text-xs px-2.5 py-1' : 'text-xs px-3 py-1.5';
  return (
    <button
      type="button"
      className={`${sizeClass} rounded-full border transition-colors ${
        selected
          ? 'bg-ink-900 text-ink-50 border-ink-900 shadow-sm'
          : 'bg-surface-elevated text-ink-700 border-fold hover:bg-ink-100 hover:border-border-accent'
      } ${className}`}
      {...rest}
    >
      {children}
    </button>
  );
}

/* ── Form fields ─────────────────────────────────────── */

interface SettingsFieldProps extends InputHTMLAttributes<HTMLInputElement> {
  label: string;
  hint?: string;
}

export function SettingsField({ label, hint, className = '', ...rest }: SettingsFieldProps) {
  return (
    <label className="block space-y-1.5">
      <span className="text-xs font-medium text-ink-700">{label}</span>
      <Input className={`text-sm text-ink-900 ${className}`} {...rest} />
      {hint && <p className="text-xs text-ink-600">{hint}</p>}
    </label>
  );
}

interface SettingsNumberFieldProps {
  label: string;
  value: number;
  onChange: (value: number) => void;
  min?: number;
}

export function SettingsNumberField({ label, value, onChange, min = 0 }: SettingsNumberFieldProps) {
  return (
    <label className="block space-y-1.5">
      <span className="text-xs font-medium text-ink-700">{label}</span>
      <Input
        type="number"
        min={min}
        value={value}
        onChange={(e) => onChange(Number(e.target.value))}
        className="text-sm text-ink-900"
      />
    </label>
  );
}

export function SettingsHint({ children }: { children: ReactNode }) {
  return <p className="text-xs text-ink-600 leading-relaxed">{children}</p>;
}

export function SettingsMuted({ children }: { children: ReactNode }) {
  return <p className="text-xs text-ink-600 italic leading-relaxed">{children}</p>;
}

/* ── Accordion ───────────────────────────────────────── */

interface SettingsAccordionProps {
  expanded: boolean;
  onToggle: () => void;
  title: ReactNode;
  meta?: ReactNode;
  leading?: ReactNode;
  children: ReactNode;
}

export function SettingsAccordion({
  expanded,
  onToggle,
  title,
  meta,
  leading,
  children,
}: SettingsAccordionProps) {
  return (
    <div className="bg-surface-elevated border border-fold rounded-lg overflow-hidden">
      <button
        type="button"
        onClick={onToggle}
        className="w-full flex items-center gap-2 px-3 py-2.5 text-sm text-ink-800 hover:bg-surface-parchment transition-colors"
      >
        <span className="text-ink-500 shrink-0 text-xs w-3">{expanded ? '▾' : '▸'}</span>
        {leading}
        <span className="flex-1 text-left font-medium">{title}</span>
        {meta && <span className="text-xs text-ink-600 shrink-0">{meta}</span>}
      </button>
      {expanded && (
        <div className="px-3 pb-3 pt-2 space-y-3 border-t border-border-subtle bg-surface-parchment/60">
          {children}
        </div>
      )}
    </div>
  );
}

/* ── Actions ─────────────────────────────────────────── */

interface SettingsActionProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: 'default' | 'danger' | 'accent';
  children: ReactNode;
}

export function SettingsAction({
  variant = 'default',
  children,
  className = '',
  ...rest
}: SettingsActionProps) {
  const variantClass =
    variant === 'danger'
      ? 'text-vermillion border-vermillion/30 hover:bg-vermillion-light'
      : variant === 'accent'
        ? 'text-jade border-jade/30 hover:bg-jade-light'
        : 'text-ink-700 border-fold hover:bg-ink-100 hover:border-border-accent';

  return (
    <button
      type="button"
      className={`text-xs px-3 py-1.5 border rounded-lg transition-colors ${variantClass} ${className}`}
      {...rest}
    >
      {children}
    </button>
  );
}

export function SettingsSaveButton({
  children,
  ...rest
}: ButtonHTMLAttributes<HTMLButtonElement>) {
  return (
    <Button variant="primary" className="text-xs px-4 py-1.5 rounded-md" {...rest}>
      {children}
    </Button>
  );
}

/* ── Toggle ──────────────────────────────────────────── */

interface SettingsToggleProps {
  checked: boolean;
  onChange: (checked: boolean) => void;
  label: string;
  onLabel?: string;
  offLabel?: string;
}

export function SettingsToggle({
  checked,
  onChange,
  label,
  onLabel = '开启',
  offLabel = '关闭',
}: SettingsToggleProps) {
  return (
    <label className="flex items-center gap-3 py-1">
      <span className="text-xs font-medium text-ink-700">{label}</span>
      <button
        type="button"
        role="switch"
        aria-checked={checked}
        onClick={() => onChange(!checked)}
        className={`relative w-9 h-5 rounded-full transition-colors shrink-0 ${
          checked ? 'bg-jade' : 'bg-ink-300'
        }`}
      >
        <span
          className={`absolute top-0.5 left-0.5 w-4 h-4 rounded-full bg-white shadow transition-transform ${
            checked ? 'translate-x-4' : ''
          }`}
        />
      </button>
      <span className="text-xs text-ink-600">{checked ? onLabel : offLabel}</span>
    </label>
  );
}

/* ── Checkbox label (for "use default") ──────────────── */

interface SettingsCheckboxProps {
  checked: boolean;
  onChange: () => void;
  label: string;
  onClick?: (e: React.MouseEvent) => void;
}

export function SettingsCheckbox({ checked, onChange, label, onClick }: SettingsCheckboxProps) {
  return (
    <label
      className="flex items-center gap-1.5 shrink-0 cursor-pointer"
      onClick={(e) => {
        onClick?.(e);
      }}
    >
      <input
        type="checkbox"
        checked={checked}
        onChange={() => onChange()}
        onClick={(e) => e.stopPropagation()}
        className="accent-ink-900 rounded"
      />
      <span className="text-xs text-ink-600 whitespace-nowrap">{label}</span>
    </label>
  );
}
