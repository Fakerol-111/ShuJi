import type { TextareaHTMLAttributes } from "react";

interface Props extends TextareaHTMLAttributes<HTMLTextAreaElement> {}

export function Textarea({ className = "", ...rest }: Props) {
  return (
    <textarea
      className={`text-body bg-surface-parchment border border-fold rounded-lg px-3 py-2
        focus:outline-none focus:border-vermillion focus:ring-1 focus:ring-vermillion/30
        placeholder:text-ink-400 w-full resize-none ${className}`}
      {...rest}
    />
  );
}
