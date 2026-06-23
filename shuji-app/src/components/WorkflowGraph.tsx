import { useEffect, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { getWorkflowGraph, listWorkflowArchives, loadWorkflowArchive } from '../api';
import type { WorkflowGraph, GraphNode, GraphEdge } from '../types';
import { getDeptMeta } from '../constants';

const NODE_W = 180;
const NODE_H = 64;
const H_GAP = 60;
const V_GAP = 80;
const PADDING = 40;

interface LayoutNode extends GraphNode {
  layer: number;
  index: number;
  x: number;
  y: number;
}

interface LayoutEdge extends GraphEdge {
  src: LayoutNode;
  dst: LayoutNode;
}

interface ArchiveEntry {
  filename: string;
  label: string;
  ts: string;
}

export default function WorkflowGraphView() {
  const { t } = useTranslation();
  const [graph, setGraph] = useState<WorkflowGraph | null>(null);
  const [archives, setArchives] = useState<ArchiveEntry[]>([]);
  const [currentSession, setCurrentSession] = useState<string | null>(null);
  const [activeArchive, setActiveArchive] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState('');
  const svgRef = useRef<SVGSVGElement>(null);
  const liveInterval = useRef<ReturnType<typeof setInterval> | null>(null);

  // ── Load archive list once ──
  useEffect(() => {
    listWorkflowArchives()
      .then((entries) => {
        setArchives(
          entries.map(([filename, label]) => {
            // Parse timestamp from filename: {ts}_{label}.json
            const ts = filename.split('_').slice(0, 2).join('_');
            return { filename, label: label || t('workflowGraph.unnamed'), ts };
          })
        );
      })
      .catch((e) => {
        console.error('加载工作流存档列表失败', e);
      });
  }, []);

  // ── Load graph (live or archived) ──
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

  // ── Live auto-refresh (only for current session) ──
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

  // ── SVG layout computation ──
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

  // ── Layered layout (same as before) ──
  // ── 双图拆分：尚书省层（规划阶段）vs 六部层（执行阶段） ──
  const upperRoles = new Set(['内阁', '中书令', '门下侍中', '尚书令']);
  const lowerRoles = new Set(['尚书令', '吏部', '兵部', '工部', '刑部', '礼部']);
  const hasShangshuling = graph?.nodes.some((n) => n.role === '尚书令');

  let upperLayout: ReturnType<typeof computeLayout> | null = null;
  let lowerLayout: ReturnType<typeof computeLayout> | null = null;

  if (graph && hasShangshuling) {
    const upperGraph = splitGraph(graph, (n) => upperRoles.has(n.role));
    const lowerGraph = splitGraph(graph, (n) => lowerRoles.has(n.role));
    upperLayout = computeLayout(upperGraph);
    lowerLayout = computeLayout(lowerGraph);
  } else {
    // 无尚书省节点 → 只有一个图
    const layoutResult = graph ? computeLayout(graph) : null;
    upperLayout = layoutResult;
  }

  return (
    <div className="h-full flex flex-col bg-surface-paper">
      {/* Header */}
      <div className="flex items-center gap-2 px-4 py-2 border-b border-fold shrink-0">
        <h2 className="font-display text-ui font-semibold text-ink-800">
          {t('workflowGraph.title')}
        </h2>
        {graph && (
          <span className="text-caption text-ink-500">
            {t('workflowGraph.stats', { nodes: graph.nodes.length, edges: graph.edges.length })}
            {upperLayout && upperLayout.totalDuration !== '' && (
              <span className="ml-2 text-ink-400">
                · {t('workflowGraph.durationPrefix')} {upperLayout.totalDuration}
              </span>
            )}
          </span>
        )}
      </div>

      <div className="flex-1 flex min-h-0 overflow-hidden">
        {/* Archive sidebar */}
        <div className="w-48 shrink-0 border-r border-fold overflow-y-auto bg-surface-parchment/50">
          <div className="px-3 py-2 border-b border-fold">
            <button
              onClick={switchToLive}
              className={`w-full text-left px-2 py-1.5 rounded text-ui transition-colors ${
                !activeArchive
                  ? 'bg-gold-light text-ink-800 font-medium border-l-2 border-gold'
                  : 'text-ink-600 hover:bg-ink-100'
              }`}
            >
              <div className="text-xs font-semibold">{t('workflowGraph.current')}</div>
              <div className="text-caption text-ink-500 truncate">
                {currentSession || t('workflowGraph.live')}
              </div>
              {!activeArchive && (
                <span className="inline-block w-1.5 h-1.5 rounded-full bg-jade animate-pulse ml-1" />
              )}
            </button>
          </div>
          <div className="px-3 py-2">
            <div className="text-caption font-semibold text-ink-500 mb-1">
              {t('workflowGraph.history')}
            </div>
            {archives.length === 0 && (
              <p className="text-caption text-ink-400 italic">{t('workflowGraph.noArchives')}</p>
            )}
            {archives.map((a) => (
              <button
                key={a.filename}
                onClick={() => loadArchivedGraph(a.filename)}
                className={`w-full text-left px-2 py-1.5 rounded text-ui transition-colors mb-0.5 ${
                  activeArchive === a.filename
                    ? 'bg-ink-100 text-ink-800 font-medium border-l-2 border-vermillion'
                    : 'text-ink-600 hover:bg-ink-100'
                }`}
              >
                <div className="text-caption truncate">{a.label || t('workflowGraph.unnamed')}</div>
                <div className="text-caption text-ink-400 font-mono text-[10px]">{a.ts}</div>
              </button>
            ))}
          </div>
        </div>

        {/* SVG canvas */}
        <div className="flex-1 overflow-auto">
          {/* 尚书省层（规划阶段） */}
          {upperLayout ? (
            <>
              {lowerLayout && (
                <div className="px-3 py-1.5 text-caption font-semibold text-ink-500 bg-surface-parchment/50 border-b border-fold sticky top-0 z-10">
                  {t('workflowGraph.upperLayer')}
                </div>
              )}
              <div className="mb-4">
                <svg
                  ref={svgRef}
                  viewBox={`0 0 ${upperLayout.svgW} ${upperLayout.svgH}`}
                  className="min-w-full"
                  style={{ width: upperLayout.svgW, height: upperLayout.svgH }}
                >
                  <defs>
                    <marker
                      id="arrowhead"
                      markerWidth="10"
                      markerHeight="7"
                      refX="10"
                      refY="3.5"
                      orient="auto"
                    >
                      <polygon points="0 0, 10 3.5, 0 7" fill="#8B7355" />
                    </marker>
                  </defs>
                  {/* Edges */}
                  {upperLayout.edges.map((e) => {
                    const sx = e.src.x + NODE_W / 2;
                    const sy = e.src.y + NODE_H;
                    const dx = e.dst.x + NODE_W / 2;
                    const dy = e.dst.y;
                    const cy = (sy + dy) / 2;
                    return (
                      <g key={`edge-${e.id}`}>
                        <path
                          d={`M ${sx} ${sy} C ${sx} ${cy}, ${dx} ${cy}, ${dx} ${dy}`}
                          fill="none"
                          stroke="#C4B8A2"
                          strokeWidth="2"
                          markerEnd="url(#arrowhead)"
                        />
                        <text
                          x={(sx + dx) / 2}
                          y={cy - 6}
                          textAnchor="middle"
                          fill="#8B7355"
                          fontSize="11"
                        >
                          {e.task_id.length > 24 ? e.task_id.slice(0, 22) + '…' : e.task_id}
                        </text>
                      </g>
                    );
                  })}
                  {/* Nodes */}
                  {upperLayout.layoutNodes.map((n) => {
                    const meta = getDeptMeta(n.role) || {
                      color: '#6b7280',
                      label: n.role,
                    };
                    const isMulti = n.instance > 1;
                    const color =
                      n.status === 'failed'
                        ? '#C41E3A'
                        : n.status === 'completed'
                          ? '#2D5A3F'
                          : n.status === 'planned'
                            ? meta.color + '80'
                            : meta.color;
                    const durStr = upperLayout.nodeDurations.get(n.id);
                    return (
                      <g key={`node-${n.id}`} className="group">
                        <title>
                          {n.role}
                          {isMulti ? `#${n.instance}` : ''}
                          {durStr ? `\n${t('workflowGraph.processingTime')}: ${durStr}` : ''}
                          {n.task_summary ? `\n${t('workflowGraph.task')}: ${n.task_summary}` : ''}
                          {n.created_at ? `\n${t('workflowGraph.start')}: ${n.created_at}` : ''}
                        </title>
                        <rect
                          x={n.x}
                          y={n.y}
                          width={NODE_W}
                          height={NODE_H}
                          rx="8"
                          ry="8"
                          fill={
                            n.status === 'failed'
                              ? '#FDE8EC'
                              : n.status === 'planned'
                                ? '#FAFAF5'
                                : '#F5F0E8'
                          }
                          stroke={color}
                          strokeWidth={n.status === 'active' ? 2 : 1}
                          strokeDasharray={n.status === 'planned' ? '5,3' : 'none'}
                          opacity={
                            n.status === 'completed' ? 0.7 : n.status === 'planned' ? 0.6 : 1
                          }
                        />
                        <text
                          x={n.x + NODE_W / 2}
                          y={n.y + 22}
                          textAnchor="middle"
                          fill="#1A1512"
                          fontSize="13"
                          fontWeight="600"
                        >
                          {n.role}
                          {isMulti ? `#${n.instance}` : ''}
                        </text>
                        <text
                          x={n.x + NODE_W / 2}
                          y={n.y + 42}
                          textAnchor="middle"
                          fill="#5C4F3E"
                          fontSize="10"
                        >
                          {n.task_summary.length > 18
                            ? n.task_summary.slice(0, 16) + '…'
                            : n.task_summary}
                        </text>
                        {/* Status dot */}
                        <circle cx={n.x + 12} cy={n.y + 12} r="4" fill={color} />
                        {/* Timestamp */}
                        <text
                          x={n.x + NODE_W - 4}
                          y={n.y + NODE_H - 4}
                          textAnchor="end"
                          fill="#A8926D"
                          fontSize="9"
                        >
                          {n.created_at}
                          {durStr ? ` · ${durStr}` : ''}
                        </text>
                      </g>
                    );
                  })}
                </svg>
              </div>

              {/* 六部层（执行阶段） */}
              {lowerLayout && (
                <div>
                  <div className="px-3 py-1.5 text-caption font-semibold text-ink-500 bg-surface-parchment/50 border-b border-fold sticky top-0 z-10">
                    {t('workflowGraph.lowerLayer')}
                  </div>
                  <svg
                    viewBox={`0 0 ${lowerLayout.svgW} ${lowerLayout.svgH}`}
                    className="min-w-full"
                    style={{ width: lowerLayout.svgW, height: lowerLayout.svgH }}
                  >
                    <defs>
                      <marker
                        id="arrowhead-lower"
                        markerWidth="10"
                        markerHeight="7"
                        refX="10"
                        refY="3.5"
                        orient="auto"
                      >
                        <polygon points="0 0, 10 3.5, 0 7" fill="#B83A3A" />
                      </marker>
                    </defs>
                    {lowerLayout.edges.map((e) => {
                      const sx = e.src.x + NODE_W / 2;
                      const sy = e.src.y + NODE_H;
                      const dx = e.dst.x + NODE_W / 2;
                      const dy = e.dst.y;
                      const cy = (sy + dy) / 2;
                      return (
                        <g key={`ledge-${e.id}`}>
                          <path
                            d={`M ${sx} ${sy} C ${sx} ${cy}, ${dx} ${cy}, ${dx} ${dy}`}
                            fill="none"
                            stroke="#B83A3A"
                            strokeWidth="2"
                            markerEnd="url(#arrowhead-lower)"
                          />
                          <text
                            x={(sx + dx) / 2}
                            y={cy - 6}
                            textAnchor="middle"
                            fill="#B83A3A"
                            fontSize="11"
                          >
                            {e.task_id.length > 24 ? e.task_id.slice(0, 22) + '…' : e.task_id}
                          </text>
                        </g>
                      );
                    })}
                    {lowerLayout.layoutNodes.map((n) => {
                      const meta = getDeptMeta(n.role) || { color: '#6b7280', label: n.role };
                      const isMulti = n.instance > 1;
                      const color =
                        n.status === 'failed'
                          ? '#C41E3A'
                          : n.status === 'completed'
                            ? '#2D5A3F'
                            : n.status === 'planned'
                              ? meta.color + '80'
                              : meta.color;
                      return (
                        <g key={`lnode-${n.id}`} className="group">
                          <title>
                            {n.role}
                            {isMulti ? `#${n.instance}` : ''}
                            {n.task_summary
                              ? `\n${t('workflowGraph.task')}: ${n.task_summary}`
                              : ''}
                            {n.created_at ? `\n${t('workflowGraph.start')}: ${n.created_at}` : ''}
                          </title>
                          <rect
                            x={n.x}
                            y={n.y}
                            width={NODE_W}
                            height={NODE_H}
                            rx="8"
                            ry="8"
                            fill={
                              n.status === 'failed'
                                ? '#FDE8EC'
                                : n.status === 'planned'
                                  ? '#FAFAF5'
                                  : '#F5F0E8'
                            }
                            stroke={color}
                            strokeWidth={n.status === 'active' ? 2 : 1}
                            strokeDasharray={n.status === 'planned' ? '5,3' : 'none'}
                            opacity={
                              n.status === 'completed' ? 0.7 : n.status === 'planned' ? 0.6 : 1
                            }
                          />
                          <text
                            x={n.x + NODE_W / 2}
                            y={n.y + 22}
                            textAnchor="middle"
                            fill="#1A1512"
                            fontSize="13"
                            fontWeight="600"
                          >
                            {n.role}
                            {isMulti ? `#${n.instance}` : ''}
                          </text>
                          <text
                            x={n.x + NODE_W / 2}
                            y={n.y + 42}
                            textAnchor="middle"
                            fill="#5C4F3E"
                            fontSize="10"
                          >
                            {n.task_summary.length > 18
                              ? n.task_summary.slice(0, 16) + '…'
                              : n.task_summary}
                          </text>
                          <circle cx={n.x + 12} cy={n.y + 12} r="4" fill={color} />
                          <text
                            x={n.x + NODE_W - 4}
                            y={n.y + NODE_H - 4}
                            textAnchor="end"
                            fill="#A8926D"
                            fontSize="9"
                          >
                            {n.created_at}
                          </text>
                        </g>
                      );
                    })}
                  </svg>
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

function timeToSecs(t: string): number {
  const parts = t.split(':');
  if (parts.length === 3) {
    return parseInt(parts[0]) * 3600 + parseInt(parts[1]) * 60 + parseInt(parts[2]);
  }
  if (parts.length === 2) {
    return parseInt(parts[0]) * 60 + parseInt(parts[1]);
  }
  return 0;
}

function fmtDuration(secs: number): string {
  if (secs < 60) return `${secs}s`;
  if (secs < 3600) return `${Math.floor(secs / 60)}m${secs % 60}s`;
  return `${Math.floor(secs / 3600)}h${Math.floor((secs % 3600) / 60)}m`;
}

// ── Split graph by node filter (for dual-diagram rendering) ──
function splitGraph(graph: WorkflowGraph, filter: (node: GraphNode) => boolean): WorkflowGraph {
  const nodeIds = new Set(graph.nodes.filter(filter).map((n) => n.id));
  return {
    session_label: graph.session_label,
    nodes: graph.nodes.filter((n) => nodeIds.has(n.id)),
    edges: graph.edges.filter((e) => nodeIds.has(e.source) && nodeIds.has(e.target)),
  };
}

// ── Layout computation (pure function) ──
function computeLayout(graph: WorkflowGraph): {
  layoutNodes: LayoutNode[];
  edges: LayoutEdge[];
  svgW: number;
  svgH: number;
  totalDuration: string;
  nodeDurations: Map<number, string>;
} {
  const nodeMap = new Map<number, GraphNode>();
  for (const n of graph.nodes) nodeMap.set(n.id, n);

  // indegree
  const indegree = new Map<number, number>();
  for (const n of graph.nodes) indegree.set(n.id, 0);
  for (const e of graph.edges) {
    indegree.set(e.target, (indegree.get(e.target) || 0) + 1);
  }

  // BFS layer assignment
  const layers: number[][] = [];
  const nodeLayer = new Map<number, number>();

  let currentLayer = graph.nodes.filter((n) => (indegree.get(n.id) || 0) === 0).map((n) => n.id);

  while (currentLayer.length > 0) {
    layers.push([...currentLayer]);
    for (const id of currentLayer) {
      nodeLayer.set(id, layers.length - 1);
    }
    const nextLayer: number[] = [];
    for (const e of graph.edges) {
      if (currentLayer.includes(e.source) && !nodeLayer.has(e.target)) {
        if (!nextLayer.includes(e.target)) nextLayer.push(e.target);
      }
    }
    for (const n of graph.nodes) {
      if (nodeLayer.has(n.id)) continue;
      const preds = graph.edges.filter((e) => e.target === n.id);
      if (preds.length > 0 && preds.every((p) => nodeLayer.has(p.source))) {
        if (!nextLayer.includes(n.id)) nextLayer.push(n.id);
      }
    }
    currentLayer = nextLayer;
  }

  for (const n of graph.nodes) {
    if (!nodeLayer.has(n.id)) {
      nodeLayer.set(n.id, layers.length - 1);
      if (layers.length === 0) layers.push([]);
      layers[layers.length - 1].push(n.id);
    }
  }

  const layoutNodes: LayoutNode[] = [];
  for (let li = 0; li < layers.length; li++) {
    const ids = layers[li].sort((a, b) => a - b);
    for (let ni = 0; ni < ids.length; ni++) {
      const n = nodeMap.get(ids[ni])!;
      layoutNodes.push({
        ...n,
        layer: li,
        index: ni,
        x: PADDING + ni * (NODE_W + H_GAP),
        y: PADDING + li * (NODE_H + V_GAP),
      });
    }
  }

  const lnMap = new Map<number, LayoutNode>();
  for (const ln of layoutNodes) lnMap.set(ln.id, ln);

  const layoutEdges: LayoutEdge[] = [];
  for (const e of graph.edges) {
    const src = lnMap.get(e.source);
    const dst = lnMap.get(e.target);
    if (src && dst) layoutEdges.push({ ...e, src, dst });
  }

  // ── 耗时计算 ──
  const nodeDurations = new Map<number, string>();
  for (const node of graph.nodes) {
    // 找到该节点的第一个出边时间
    const outEdge = graph.edges
      .filter((e) => e.source === node.id)
      .sort((a, b) => timeToSecs(a.timestamp) - timeToSecs(b.timestamp))[0];
    if (outEdge && node.created_at) {
      const startSecs = timeToSecs(node.created_at);
      const endSecs = timeToSecs(outEdge.timestamp);
      if (endSecs > startSecs) {
        nodeDurations.set(node.id, fmtDuration(endSecs - startSecs));
      }
    }
  }

  // 总耗时：最新边 - 最早边
  let totalDuration = '';
  if (graph.edges.length > 0) {
    const allTimes = graph.edges.map((e) => timeToSecs(e.timestamp)).sort((a, b) => a - b);
    const diff = allTimes[allTimes.length - 1] - allTimes[0];
    if (diff > 0) totalDuration = fmtDuration(diff);
  }

  const maxLayerWidth = Math.max(...layers.map((l) => l.length), 1);
  const svgW = PADDING * 2 + maxLayerWidth * (NODE_W + H_GAP) - H_GAP;
  const svgH = PADDING * 2 + layers.length * (NODE_H + V_GAP) - V_GAP;

  return {
    layoutNodes,
    edges: layoutEdges,
    svgW,
    svgH,
    totalDuration,
    nodeDurations,
  };
}
