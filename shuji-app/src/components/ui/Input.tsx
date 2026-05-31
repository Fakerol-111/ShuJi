import type { InputHTMLAttributes } from "react";

interface Props extends InputHTMLAttributes<HTMLInputElement> {}

export function Input({ className = "", ...rest }: Props) {
  return (
    <input
      className={`text-body bg-surface-parchment border border-fold rounded-lg px-3 py-2
        focus:outline-none focus:border-vermillion focus:ring-1 focus:ring-vermillion/30
        placeholder:text-ink-400 w-full ${className}`}
      {...rest}
    />
  );
}
