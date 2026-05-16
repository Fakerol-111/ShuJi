import { useState, useEffect, useRef } from "react";
import { useNavigate } from "react-router-dom";
import { open } from "@tauri-apps/plugin-dialog";
import { loadProject, getRecentDirs, getConfig } from "../api";

export default function WorkspaceSelect() {
  const navigate = useNavigate();
  const [recentDirs, setRecentDirs] = useState<string[]>([]);
  const [dirPath, setDirPath] = useState("");
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState("");
  const dirInputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    // Check if API keys are configured; redirect to setup on first run
    getConfig()
      .then((cfg) => {
        const def = cfg.roles?.default;
        if (!def?.api_key) {
          navigate("/setup", { replace: true });
          return;
        }
      })
      .catch(() => {});
    getRecentDirs().then(setRecentDirs).catch(() => {});
  }, []);

  const handleBrowse = async () => {
    try {
      const selected = await open({
        directory: true,
        multiple: false,
        title: "选择工作目录",
      });
      if (selected) setDirPath(selected);
    } catch { /* cancelled */ }
  };

  const handleOpen = async (dir?: string) => {
    const path = dir || dirPath.trim();
    if (!path) { setError("请选择工作目录"); return; }
    setLoading(true);
    setError("");
    try {
      await loadProject(path);
      navigate("/project");
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  };

  return (
    <div className="min-h-screen bg-ink-50 flex items-center justify-center">
      <div className="bg-white rounded-xl shadow-lg shadow-ink-200/50 border border-ink-200 w-full max-w-md p-8">
        <div className="text-center mb-8">
          <h1 className="text-3xl font-bold text-ink-900 mb-2 tracking-wide">枢机</h1>
          <p className="text-ink-500 text-sm">三省六部制自动化软件开发系统</p>
        </div>

        <div className="mb-4">
          <label className="block text-xs font-medium text-ink-500 mb-1 tracking-wide">
            工作目录
          </label>
          <div className="flex gap-2">
            <input
              ref={dirInputRef}
              type="text"
              value={dirPath}
              onChange={(e) => setDirPath(e.target.value)}
              placeholder="选择一个文件夹..."
              className="flex-1 px-3 py-2 border border-ink-200 bg-ink-50 rounded-lg text-sm font-mono text-ink-800 placeholder:text-ink-400 focus:outline-none focus:border-ink-500"
            />
            <button
              onClick={handleBrowse}
              className="px-3 py-2 border border-ink-200 rounded-lg text-ink-600 hover:bg-ink-100 text-sm transition-colors"
            >
              浏览
            </button>
            <button
              onClick={() => handleOpen()}
              disabled={loading}
              className="px-4 py-2 bg-ink-900 text-ink-50 rounded-lg hover:bg-ink-800 disabled:opacity-40 text-sm transition-colors"
            >
              {loading ? "打开中..." : "打开"}
            </button>
          </div>
        </div>

        {recentDirs.length > 0 && (
          <div>
            <label className="block text-xs font-medium text-ink-400 mb-1 tracking-wide">
              最近目录
            </label>
            <div className="space-y-0.5">
              {recentDirs.map((dir, i) => (
                <button
                  key={i}
                  onClick={() => handleOpen(dir)}
                  className="block w-full text-left px-3 py-1.5 text-sm text-ink-600 hover:bg-ink-100 rounded truncate transition-colors"
                >
                  {dir}
                </button>
              ))}
            </div>
          </div>
        )}

        {error && (
          <div className="mt-3 text-sm text-vermillion-dark bg-vermillion-light border border-vermillion/20 p-2 rounded">{error}</div>
        )}
      </div>
    </div>
  );
}
