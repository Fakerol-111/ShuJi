interface SoulSettingsTabProps {
  setSavedMsg: (msg: string) => void;
}

export default function SoulSettingsTab({ setSavedMsg }: SoulSettingsTabProps) {
  return (
    <div className="space-y-1">
      <span className="text-[11px] font-semibold text-ink-300">Soul 管理</span>
      <div className="flex gap-2 flex-wrap pt-1">
        <button
          onClick={async () => {
            try {
              const { getSoulContent } = await import('../../api');
              const content = await getSoulContent();
              if (!content) {
                setSavedMsg('soul 为空或不存在');
                setTimeout(() => setSavedMsg(''), 2000);
                return;
              }
              await navigator.clipboard.writeText(content);
              setSavedMsg('soul 已复制到剪贴板');
              setTimeout(() => setSavedMsg(''), 2000);
            } catch (e) {
              setSavedMsg(String(e));
            }
          }}
          className="text-[10px] px-2 py-1 text-ink-400 hover:text-ink-200 border border-ink-700 hover:border-ink-500 rounded transition-colors"
        >
          导出 soul（复制）
        </button>
        <button
          onClick={async () => {
            try {
              const { clearSoul } = await import('../../api');
              await clearSoul();
              setSavedMsg('soul 已重置为默认');
              setTimeout(() => setSavedMsg(''), 2000);
            } catch (e) {
              setSavedMsg(String(e));
            }
          }}
          className="text-[10px] px-2 py-1 text-red-400 hover:text-red-300 border border-red-800 hover:border-red-600 rounded transition-colors"
        >
          清空 soul
        </button>
      </div>
      <div className="text-[10px] text-ink-500 px-1">
        soul 超 8KB 时将自动压缩。单条经验/教训/偏好 ≤500 字符。
      </div>
    </div>
  );
}
