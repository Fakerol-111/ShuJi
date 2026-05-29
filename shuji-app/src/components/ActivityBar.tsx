export type ActivitySelection = "files" | "stats" | "context" | null;

interface ActivityBarProps {
  selected: ActivitySelection;
  onSelect: (selected: ActivitySelection) => void;
  onLogsClick: () => void;
}

const ITEMS: Array<{ id: Exclude<ActivitySelection, null>; icon: string; label: string }> = [
  { id: "files", icon: "📁", label: "文件" },
  { id: "stats", icon: "📊", label: "度支" },
  { id: "context", icon: "📝", label: "文脉" },
];

export default function ActivityBar({ selected, onSelect, onLogsClick }: ActivityBarProps) {
  return (
    <div className="w-12 bg-ink-900 border-r border-ink-800 flex flex-col items-center py-2 shrink-0">
      {ITEMS.map((item) => {
        const active = selected === item.id;
        return (
          <button
            key={item.id}
            title={item.label}
            onClick={() => onSelect(active ? null : item.id)}
            className={`relative w-full h-11 flex items-center justify-center text-lg transition-colors ${
              active ? "bg-ink-800 text-ink-50" : "text-ink-500 hover:text-ink-200 hover:bg-ink-800/60"
            }`}
          >
            {active && <span className="absolute left-0 top-1 bottom-1 w-0.5 bg-vermillion rounded-r" />}
            <span aria-hidden>{item.icon}</span>
          </button>
        );
      })}
      <button
        title="日志"
        onClick={onLogsClick}
        className="mt-1 w-full h-11 flex items-center justify-center text-lg text-ink-500 hover:text-ink-200 hover:bg-ink-800/60 transition-colors"
      >
        📋
      </button>
    </div>
  );
}
