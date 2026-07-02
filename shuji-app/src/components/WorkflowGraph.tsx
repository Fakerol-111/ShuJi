import { useTranslation } from 'react-i18next';
import { computeLayout, splitGraph } from './workflow-graph/layout';
import { useWorkflowGraphData } from './workflow-graph/useWorkflowGraphData';
import ArchiveSidebar from './workflow-graph/ArchiveSidebar';
import GraphSection from './workflow-graph/GraphSection';

export default function WorkflowGraphView() {
  const { t } = useTranslation();
  const {
    graph,
    archives,
    activeArchive,
    currentSession,
    loading,
    error,
    loadLiveGraph,
    loadArchivedGraph,
    switchToLive,
  } = useWorkflowGraphData();

  if (loading && !graph) {
    return (
      <div className="h-full flex items-center justify-center text-ink-400">
        {t('workflowGraph.loading')}
      </div>
    );
  }

  if (error) {
    return (
      <div className="h-full flex flex-col items-center justify-center gap-3">
        <p className="text-vermillion">{error}</p>
        <button onClick={loadLiveGraph} className="text-ui text-gold underline">
          {t('common.retry')}
        </button>
      </div>
    );
  }

  const upperRoles = new Set(['内阁', '中书令', '门下侍中', '尚书令']);
  const lowerRoles = new Set(['尚书令', '吏部', '兵部', '工部', '刑部', '礼部']);
  const hasShangshuling = graph?.nodes.some((n) => n.role === '尚书令');
  const upperLayout =
    graph && hasShangshuling
      ? computeLayout(splitGraph(graph, (n) => upperRoles.has(n.role)))
      : graph
        ? computeLayout(graph)
        : null;
  const lowerLayout =
    graph && hasShangshuling
      ? computeLayout(splitGraph(graph, (n) => lowerRoles.has(n.role)))
      : null;

  return (
    <div className="h-full flex flex-col bg-surface-paper">
      <div className="flex items-center gap-2 px-4 py-2 border-b border-fold shrink-0">
        <h2 className="font-display text-ui font-semibold text-ink-800">
          {t('workflowGraph.title')}
        </h2>
        {graph && (
          <span className="text-caption text-ink-500">
            {t('workflowGraph.stats', { nodes: graph.nodes.length, edges: graph.edges.length })}
            {upperLayout && upperLayout.totalDuration && (
              <span className="ml-2 text-ink-400">
                · {t('workflowGraph.durationPrefix')} {upperLayout.totalDuration}
              </span>
            )}
          </span>
        )}
      </div>

      <div className="flex-1 flex min-h-0 overflow-hidden">
        <ArchiveSidebar
          archives={archives}
          activeArchive={activeArchive}
          currentSession={currentSession}
          switchToLive={switchToLive}
          loadArchivedGraph={loadArchivedGraph}
          t={t}
        />

        <div className="flex-1 overflow-auto">
          {upperLayout ? (
            <>
              {lowerLayout && (
                <div className="px-3 py-1.5 text-caption font-semibold text-ink-500 bg-surface-parchment/50 border-b border-fold sticky top-0 z-10">
                  {t('workflowGraph.upperLayer')}
                </div>
              )}
              <div className="mb-4">
                <GraphSection layout={upperLayout} isLower={false} t={t} />
              </div>
              {lowerLayout && (
                <div>
                  <div className="px-3 py-1.5 text-caption font-semibold text-ink-500 bg-surface-parchment/50 border-b border-fold sticky top-0 z-10">
                    {t('workflowGraph.lowerLayer')}
                  </div>
                  <GraphSection layout={lowerLayout} isLower={true} t={t} />
                </div>
              )}
            </>
          ) : (
            <div className="h-full flex items-center justify-center text-ink-400">
              {t('workflowGraph.noRecords')}
            </div>
          )}
        </div>
      </div>
    </div>
  );
}
