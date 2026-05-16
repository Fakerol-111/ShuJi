import { useState, useEffect } from "react";
import { useNavigate } from "react-router-dom";
import { listLogFiles, readLogFile } from "../api";

interface Props {
  onClose?: () => void;
}

export default function LogsPage({ onClose }: Props) {
  const navigate = useNavigate();
  const [logFiles, setLogFiles] = useState<string[]>([]);
  const [selectedLog, setSelectedLog] = useState<string | null>(null);
  const [logContent, setLogContent] = useState<string[]>([]);

  const refresh = async () => {
    const files = await listLogFiles();
    setLogFiles(files);
    if (selectedLog && files.includes(selectedLog)) {
      setLogContent(await readLogFile(selectedLog));
    }
  };

  useEffect(() => { refresh(); }, []);

  const selectFile = async (f: string) => {
    setSelectedLog(f);
    setLogContent(await readLogFile(f));
  };

  const close = onClose || (() => navigate("/"));

  const inner = (
    <div className="h-screen bg-ink-50 flex flex-col">
      <header className="bg-ink-900 border-b border-ink-800 shrink-0">
        <div className="px-5 py-2.5 flex items-center justify-between">
          <h1 className="text-base font-bold text-ink-50 tracking-wide">日志</h1>
          <div className="flex items-center gap-1.5">
            <button onClick={refresh} className="text-xs px-2.5 py-1.5 text-ink-400 hover:text-ink-200 hover:bg-ink-800 rounded transition-colors">刷新</button>
            <button onClick={close} className="text-xs px-2.5 py-1.5 text-ink-400 hover:text-ink-200 hover:bg-ink-800 rounded transition-colors">← 返回</button>
          </div>
        </div>
      </header>
      <div className="flex-1 px-5 py-4 flex gap-4 min-h-0 overflow-hidden">
        <div className="w-48 shrink-0 space-y-0.5 overflow-y-auto">
          {logFiles.map((f) => (
            <button
              key={f}
              onClick={() => selectFile(f)}
              className={`block w-full text-left text-xs px-3 py-1.5 rounded transition-colors ${
                selectedLog === f
                  ? "bg-ink-900 text-ink-50"
                  : "bg-white text-ink-700 hover:bg-ink-100 border border-ink-200"
              }`}
            >
              {f.replace(".jsonl", "")}
            </button>
          ))}
        </div>
        <div className="flex-1 bg-ink-900 text-ink-200 rounded-lg p-4 overflow-y-auto font-mono">
          {!selectedLog && <p className="text-xs text-ink-600">选择左侧日志文件查看</p>}
          {logContent.map((line, i) => (
            <pre key={i} className="text-xs leading-relaxed whitespace-pre-wrap">
              {(() => { try { return JSON.stringify(JSON.parse(line), null, 1); } catch { return line; } })()}
            </pre>
          ))}
        </div>
      </div>
    </div>
  );

  // Overlay mode: fixed full-screen on top of everything
  if (onClose) {
    return <div className="fixed inset-0 z-50">{inner}</div>;
  }

  // Standalone page
  return inner;
}
