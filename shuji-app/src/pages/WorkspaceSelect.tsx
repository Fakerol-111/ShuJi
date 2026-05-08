import { useState, useEffect, useRef } from "react";
import { useNavigate } from "react-router-dom";
import { open } from "@tauri-apps/plugin-dialog";
import { loadProject, getRecentDirs } from "../api";

export default function WorkspaceSelect() {
  const navigate = useNavigate();
  const [recentDirs, setRecentDirs] = useState<string[]>([]);
  const [dirPath, setDirPath] = useState("");
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState("");
  const dirInputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
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
    <div className="min-h-screen bg-gray-50 flex items-center justify-center">
      <div className="bg-white rounded-xl shadow-lg w-full max-w-md p-8">
        <div className="text-center mb-8">
          <h1 className="text-3xl font-bold text-gray-900 mb-2">枢机</h1>
          <p className="text-gray-500">三省六部制自动化软件开发系统</p>
        </div>

        <div className="mb-4">
          <label className="block text-sm font-medium text-gray-700 mb-1">
            工作目录
          </label>
          <div className="flex gap-2">
            <input
              ref={dirInputRef}
              type="text"
              value={dirPath}
              onChange={(e) => setDirPath(e.target.value)}
              placeholder="选择一个文件夹..."
              className="flex-1 px-3 py-2 border border-gray-300 rounded-lg text-sm font-mono"
            />
            <button
              onClick={handleBrowse}
              className="px-3 py-2 border border-gray-300 rounded-lg hover:bg-gray-100 text-sm"
            >
              浏览
            </button>
            <button
              onClick={() => handleOpen()}
              disabled={loading}
              className="px-4 py-2 bg-blue-600 text-white rounded-lg hover:bg-blue-700 disabled:opacity-50 text-sm"
            >
              {loading ? "打开中..." : "打开"}
            </button>
          </div>
        </div>

        {recentDirs.length > 0 && (
          <div>
            <label className="block text-sm font-medium text-gray-500 mb-1">
              最近目录
            </label>
            <div className="space-y-1">
              {recentDirs.map((dir, i) => (
                <button
                  key={i}
                  onClick={() => handleOpen(dir)}
                  className="block w-full text-left px-3 py-2 text-sm text-blue-600 hover:bg-blue-50 rounded truncate"
                >
                  {dir}
                </button>
              ))}
            </div>
          </div>
        )}

        {error && (
          <div className="mt-3 text-sm text-red-600 bg-red-50 p-2 rounded">{error}</div>
        )}
      </div>
    </div>
  );
}
