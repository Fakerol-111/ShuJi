interface ProjectPickerProps {
  recentDirs: string[];
  pickerPath: string;
  pickerError: string;
  pickerLoading: boolean;
  setPickerPath: (value: string) => void;
  onBrowse: () => void;
  onLoad: (dir?: string) => void;
  onClose: () => void;
}

export default function ProjectPicker(props: ProjectPickerProps) {
  const { recentDirs, pickerPath, pickerError, pickerLoading, setPickerPath, onBrowse, onLoad, onClose } = props;
  return (
    <div className="fixed inset-0 bg-black/50 z-50 flex items-center justify-center" onClick={onClose}>
      <div className="bg-white rounded-xl shadow-2xl border border-ink-200 w-full max-w-md p-6 mx-4" onClick={(e) => e.stopPropagation()}>
        <h2 className="text-lg font-bold text-ink-900 mb-4">加载项目</h2>
        <label className="block text-xs font-medium text-ink-500 mb-1">工作目录</label>
        <div className="flex gap-2 mb-3">
          <input
            value={pickerPath}
            onChange={(e) => setPickerPath(e.target.value)}
            onKeyDown={(e) => { if (e.key === "Enter") onLoad(); }}
            placeholder="选择一个文件夹..."
            className="flex-1 px-3 py-2 border border-ink-200 bg-ink-50 rounded-lg text-sm font-mono text-ink-800 focus:outline-none focus:border-ink-500"
          />
          <button onClick={onBrowse} className="px-3 py-2 border border-ink-200 rounded-lg text-ink-600 hover:bg-ink-100 text-sm">浏览</button>
        </div>
        {recentDirs.length > 0 && (
          <div className="mb-3">
            <div className="text-xs text-ink-400 mb-1">最近</div>
            <div className="space-y-0.5 max-h-32 overflow-y-auto">
              {recentDirs.map((d, i) => (
                <button key={i} onClick={() => onLoad(d)} className="block w-full text-left px-2 py-1 text-xs text-ink-600 hover:bg-ink-100 rounded truncate">{d}</button>
              ))}
            </div>
          </div>
        )}
        {pickerError && <div className="text-xs text-vermillion-dark bg-vermillion-light border border-vermillion/20 p-2 rounded mb-3">{pickerError}</div>}
        <div className="flex gap-2 justify-end">
          <button onClick={onClose} className="px-4 py-2 text-sm text-ink-500 hover:text-ink-700 border border-ink-200 rounded-lg">取消</button>
          <button onClick={() => onLoad()} disabled={pickerLoading} className="px-4 py-2 text-sm bg-ink-900 text-white rounded-lg hover:bg-ink-800 disabled:opacity-40">{pickerLoading ? "加载中..." : "打开"}</button>
        </div>
      </div>
    </div>
  );
}
