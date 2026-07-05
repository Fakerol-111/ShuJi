/**
 * Project Context — centralizes project state that was previously prop-drilled.
 *
 * Holds: project, recentDirs, error, loadProjectIntoState.
 * useChat and useDocumentTabs remain in ProjectDashboard (lifecycle-bound).
 */
import { createContext, useContext, useState, useEffect, useCallback, type ReactNode } from 'react';
import { useTranslation } from 'react-i18next';
import { useNavigate } from 'react-router-dom';
import { getConfig, loadProject, getRecentDirs, getChatHistory, onProjectUpdate } from '../api';
import type { Project } from '../types';

export interface ProjectContextValue {
  project: Project | null;
  setProject: (p: Project | null) => void;
  recentDirs: string[];
  setRecentDirs: (dirs: string[]) => void;
  error: string;
  setError: (e: string) => void;
  loadProjectIntoState: (path: string) => Promise<void>;
}

const ProjectContext = createContext<ProjectContextValue>({
  project: null,
  setProject: () => {},
  recentDirs: [],
  setRecentDirs: () => {},
  error: '',
  setError: () => {},
  loadProjectIntoState: async () => {},
});

export function useProjectContext() {
  return useContext(ProjectContext);
}

export function ProjectProvider({ children }: { children: ReactNode }) {
  const navigate = useNavigate();
  const { t } = useTranslation();
  const [project, setProject] = useState<Project | null>(null);
  const [recentDirs, setRecentDirs] = useState<string[]>([]);
  const [error, setError] = useState('');

  useEffect(() => {
    getConfig()
      .then((cfg) => {
        if (!cfg.roles?.default?.api_key) navigate('/setup', { replace: true });
      })
      .catch((e) => setError(t('project.loadConfigFailed', { error: e })));
    getRecentDirs()
      .then((dirs) => {
        setRecentDirs(dirs);
        if (dirs.length > 0)
          void loadProjectIntoState(dirs[0]).catch((e) =>
            setError(e instanceof Error ? e.message : String(e))
          );
      })
      .catch((e) => setError(t('project.loadRecentFailed', { error: e })));
  }, []);

  const loadProjectIntoState = useCallback(async (path: string) => {
    const p = await loadProject(path);
    setProject(p);
    // Load chat history (preserves original useProject behavior)
    await getChatHistory();
  }, []);

  // Listen for backend project-update events
  useEffect(() => {
    const unlisten = onProjectUpdate((payload: Project) => {
      setProject(payload);
    });
    return () => {
      unlisten.then((f) => f());
    };
  }, []);

  const value: ProjectContextValue = {
    project,
    setProject,
    recentDirs,
    setRecentDirs,
    error,
    setError,
    loadProjectIntoState,
  };

  return <ProjectContext.Provider value={value}>{children}</ProjectContext.Provider>;
}
