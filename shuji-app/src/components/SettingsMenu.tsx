interface SettingsMenuProps {
  onOpenSettings: () => void;
}

export default function SettingsMenu({ onOpenSettings }: SettingsMenuProps) {
  return (
    <button
      onClick={onOpenSettings}
      className="text-xs px-2 py-1 text-ink-400 hover:text-ink-100 hover:bg-ink-800 rounded"
    >
      ⚙ 设置
    </button>
  );
}
