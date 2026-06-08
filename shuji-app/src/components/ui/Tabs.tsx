import type { ReactNode } from "react";

interface Tab {
  key: string;
  label: string;
}

interface Props {
  tabs: Tab[];
  activeKey: string;
  onChange: (key: string) => void;
  className?: string;
  extra?: ReactNode;
}

export function Tabs({
  tabs,
  activeKey,
  onChange,
  className = "",
  extra,
}: Props) {
  return (
    <div className={`flex items-center gap-2 ${className}`} role="tablist">
      <div className="bg-surface-parchment rounded-lg p-1 flex gap-1">
        {tabs.map((tab) => (
          <button
            key={tab.key}
            onClick={() => onChange(tab.key)}
            role="tab"
            aria-selected={activeKey === tab.key}
            className={`text-ui px-3 py-1.5 rounded-md font-medium transition-colors
              ${
                activeKey === tab.key
                  ? "bg-ink-900 text-ink-50 shadow-sm"
                  : "text-ink-600 hover:text-ink-900"
              }`}
          >
            {tab.label}
          </button>
        ))}
      </div>
      {extra && <div className="ml-auto">{extra}</div>}
    </div>
  );
}
