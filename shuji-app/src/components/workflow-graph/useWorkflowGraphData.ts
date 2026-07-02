import { useEffect, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { getWorkflowGraph, listWorkflowArchives, loadWorkflowArchive } from '../../api';
import type { WorkflowGraph } from '../../types';
import type { ArchiveEntry } from './types';

export function useWorkflowGraphData() {
  const { t } = useTranslation();
  const [graph, setGraph] = useState<WorkflowGraph | null>(null);
  const [archives, setArchives] = useState<ArchiveEntry[]>([]);
  const [currentSession, setCurrentSession] = useState<string | null>(null);
  const [activeArchive, setActiveArchive] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState('');
  const liveInterval = useRef<ReturnType<typeof setInterval> | null>(null);

  useEffect(() => {
    listWorkflowArchives()
      .then((entries) => {
        setArchives(
          entries.map(([filename, label]) => {
            const ts = filename.split('_').slice(0, 2).join('_');
            return { filename, label: label || t('workflowGraph.unnamed'), ts };
          })
        );
      })
      .catch(() => {});
  }, []);

  const loadLiveGraph = () => {
    getWorkflowGraph()
      .then((g) => {
        if (g) {
          setGraph(g);
          setCurrentSession(g.session_label || t('workflowGraph.current'));
        }
        setLoading(false);
      })
      .catch((e) => {
        setError(String(e));
        setLoading(false);
      });
  };

  const loadArchivedGraph = (filename: string) => {
    setLoading(true);
    setActiveArchive(filename);
    loadWorkflowArchive(filename)
      .then((g) => {
        setGraph(g);
        setLoading(false);
      })
      .catch((e) => {
        setError(String(e));
        setLoading(false);
      });
  };

  const switchToLive = () => {
    setActiveArchive(null);
    loadLiveGraph();
  };

  useEffect(() => {
    if (activeArchive) {
      if (liveInterval.current) clearInterval(liveInterval.current);
      return;
    }
    loadLiveGraph();
    liveInterval.current = setInterval(loadLiveGraph, 5000);
    return () => {
      if (liveInterval.current) clearInterval(liveInterval.current);
    };
  }, [activeArchive]);

  return {
    graph,
    archives,
    currentSession,
    activeArchive,
    loading,
    error,
    loadLiveGraph,
    loadArchivedGraph,
    switchToLive,
  };
}
