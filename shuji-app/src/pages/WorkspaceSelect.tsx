import { useState, useEffect, useRef } from 'react';
import { useNavigate } from 'react-router-dom';
import { open } from '@tauri-apps/plugin-dialog';
import { formatError } from '../utils/error';
import { loadProject, getRecentDirs, getConfig, createDemoProject } from '../api';
import { SealLogo } from '../components/SealLogo';
import { Card } from '../components/ui/Card';
import { Button } from '../components/ui/Button';

export default function WorkspaceSelect() {
  const navigate = useNavigate();
  const [recentDirs, setRecentDirs] = useState<string[]>([]);
  const [dirPath, setDirPath] = useState('');
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState('');
  const dirInputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    // Check if API keys are configured; redirect to setup on first run
    getConfig()
      .then((cfg) => {
        const def = cfg.roles?.default;
        if (!def?.api_key) {
          navigate('/setup', { replace: true });
          return;
        }
      })
      .catch((e) => console.error('读取配置失败:', e));
    getRecentDirs()
      .then(setRecentDirs)
      .catch((e) => console.error('读取最近目录失败:', e));
  }, []);

  const handleBrowse = async () => {
    try {
      const selected = await open({
        directory: true,
        multiple: false,
        title: '选择工作目录',
      });
      if (selected) setDirPath(selected);
    } catch {
      /* cancelled */
    }
  };

  const handleOpen = async (dir?: string) => {
    const path = dir || dirPath.trim();
    if (!path) {
      setError('请选择工作目录');
      return;
    }
    setLoading(true);
    setError('');
    try {
      await loadProject(path);
      navigate('/project');
    } catch (e) {
      setError(formatError(e));
    } finally {
      setLoading(false);
    }
  };

  return (
    <div className="min-h-screen bg-surface-paper flex items-center justify-center">
      <Card variant="paper" className="w-full max-w-md p-8">
        <div className="text-center mb-6">
          <div className="flex justify-center mb-3">
            <SealLogo size={40} />
          </div>
          <h1 className="font-display text-display font-bold text-ink-900 mb-2">枢机</h1>
          <p className="text-body text-ink-600">三省六部制自动化软件开发系统</p>
        </div>

        {/* ── Quick-start for new users ── */}
        <Button
          variant="seal"
          className="w-full mb-4 !py-3 !text-ui font-bold"
          disabled={loading}
          onClick={async () => {
            setLoading(true);
            setError('');
            try {
              const project = await createDemoProject();
              await loadProject(project.working_dir);
              sessionStorage.setItem('shuji_demo', 'true');
              navigate('/project');
            } catch (e) {
              setError(formatError(e));
              setLoading(false);
            }
          }}
        >
          {loading ? '创建中...' : '体验枢机 — 5 分钟上手'}
        </Button>

        <div className="relative mb-4">
          <div className="absolute inset-0 flex items-center">
            <div className="w-full border-t border-fold" />
          </div>
          <div className="relative flex justify-center text-ui">
            <span className="bg-surface-elevated px-2 text-ink-400">或打开已有项目</span>
          </div>
        </div>

        <div className="mb-4">
          <label className="block text-ui font-medium text-ink-500 mb-1 tracking-wide">
            工作目录
          </label>
          <div className="flex gap-2">
            <input
              ref={dirInputRef}
              type="text"
              value={dirPath}
              onChange={(e) => setDirPath(e.target.value)}
              placeholder="选择一个文件夹..."
              className="flex-1 px-3 py-2 border border-fold bg-surface-parchment rounded-lg text-body font-mono text-ink-800 placeholder:text-ink-400 focus:outline-none focus:border-vermillion focus:ring-1 focus:ring-vermillion/30"
            />
            <button
              onClick={handleBrowse}
              className="px-3 py-2 border border-fold rounded-lg text-ink-600 hover:bg-ink-100 text-ui transition-colors"
            >
              浏览
            </button>
            <button
              onClick={() => handleOpen()}
              disabled={loading}
              className="px-4 py-2 bg-ink-900 text-ink-50 rounded-lg hover:bg-ink-800 disabled:opacity-40 text-ui transition-colors"
            >
              {loading ? '打开中...' : '打开'}
            </button>
          </div>
        </div>

        {recentDirs.length > 0 && (
          <div>
            <label className="block text-ui font-medium text-ink-400 mb-1 tracking-wide">
              最近目录
            </label>
            <div className="space-y-0.5">
              {recentDirs.map((dir, i) => (
                <button
                  key={i}
                  onClick={() => handleOpen(dir)}
                  className="block w-full text-left px-3 py-1.5 text-ui text-ink-600 hover:bg-ink-100 rounded truncate transition-colors"
                >
                  {dir}
                </button>
              ))}
            </div>
          </div>
        )}

        {error && (
          <div className="mt-3 text-ui text-vermillion-dark bg-vermillion-light border border-vermillion/20 p-2 rounded">
            {error}
          </div>
        )}
      </Card>
    </div>
  );
}
