import { useState, useEffect } from 'react';
import { useNavigate } from 'react-router-dom';
import { getConfig, loadProject, getRecentDirs, getChatHistory } from '../api';
import { initialCabinetMessage } from '../utils/chat';
import type { Project, ChatMessage } from '../types';

export function useProject() {
  const navigate = useNavigate();
  const [project, setProject] = useState<Project | null>(null);
  const [recentDirs, setRecentDirs] = useState<string[]>([]);
  const [messages, setMessages] = useState<ChatMessage[]>([]);
  const [error, setError] = useState('');

  useEffect(() => {
    getConfig()
      .then((cfg) => {
        if (!cfg.roles?.default?.api_key) navigate('/setup', { replace: true });
      })
      .catch((e) => setError(`读取配置失败：${e}`));
    getRecentDirs()
      .then((dirs) => {
        setRecentDirs(dirs);
        if (dirs.length > 0)
          void loadProjectIntoState(dirs[0]).catch((e) =>
            setError(e instanceof Error ? e.message : String(e))
          );
      })
      .catch((e) => setError(`读取最近项目失败：${e}`));
  }, []);

  const loadProjectIntoState = async (path: string) => {
    const p = await loadProject(path);
    setProject(p);
    const hist = await getChatHistory();
    setMessages(hist.length > 0 ? hist : [initialCabinetMessage('有什么需要做的？请告诉我。')]);
  };

  return {
    project,
    setProject,
    messages,
    setMessages,
    recentDirs,
    setRecentDirs,
    error,
    setError,
    loadProjectIntoState,
  };
}
