import { useState, useEffect } from "react";
import { useNavigate } from "react-router-dom";
import { listLogFiles, readLogFile } from "../api";

export default function LogsPage() {
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

  return (
    <div className="h-screen bg-gray-50 flex flex-col">
      <header className="bg-white border-b shadow-sm shrink-0">
        <div className="max-w-6xl mx-auto px-6 py-3 flex items-center justify-between">
          <h1 className="text-lg font-bold text-gray-900">日志</h1>
          <div className="flex items-center gap-2">
            <button onClick={refresh} className="text-sm px-3 py-1.5 border border-gray-300 rounded hover:bg-gray-50">刷新</button>
            <button onClick={() => navigate("/project")} className="text-sm text-gray-500 hover:text-gray-700">← 返回</button>
          </div>
        </div>
      </header>
      <div className="flex-1 max-w-6xl mx-auto w-full px-6 py-4 flex gap-4 min-h-0 overflow-hidden">
        <div className="w-48 shrink-0 space-y-1 overflow-y-auto">
          {logFiles.map((f) => (
            <button
              key={f}
              onClick={() => selectFile(f)}
              className={`block w-full text-left text-xs px-3 py-2 rounded ${
                selectedLog === f ? "bg-gray-800 text-white" : "bg-white text-gray-700 hover:bg-gray-100 border"
              }`}
            >
              {f.replace(".jsonl", "")}
            </button>
          ))}
        </div>
        <div className="flex-1 bg-gray-900 text-gray-100 rounded-lg p-4 overflow-y-auto">
          {!selectedLog && <p className="text-xs text-gray-500">选择左侧日志文件查看</p>}
          {logContent.map((line, i) => (
            <pre key={i} className="text-xs leading-relaxed font-mono whitespace-pre-wrap">
              {(() => { try { return JSON.stringify(JSON.parse(line), null, 1); } catch { return line; } })()}
            </pre>
          ))}
        </div>
      </div>
    </div>
  );
}
