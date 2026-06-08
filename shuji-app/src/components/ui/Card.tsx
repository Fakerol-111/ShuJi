import type { ReactNode } from "react";

type Variant = "paper" | "parchment" | "ink";

interface Props {
  variant?: Variant;
  children: ReactNode;
  className?: string;
}

const variantStyles: Record<Variant, string> = {
  paper: "bg-surface-elevated border border-fold rounded-xl shadow-sm",
  parchment: "bg-surface-parchment border border-fold rounded-xl",
  ink: "bg-ink-900 text-ink-50 rounded-xl",
};

export function Card({ variant = "paper", children, className = "" }: Props) {
  return (
    <div className={`${variantStyles[variant]} ${className}`}>{children}</div>
  );
}
