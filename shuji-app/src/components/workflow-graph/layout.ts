import type { WorkflowGraph, GraphNode } from '../../types';
import type { LayoutNode, LayoutEdge, LayoutResult } from './types';

export const NODE_W = 180;
export const NODE_H = 64;
export const H_GAP = 60;
export const V_GAP = 80;
export const PADDING = 40;

export function timeToSecs(t: string): number {
  const parts = t.split(':');
  if (parts.length === 3)
    return parseInt(parts[0]) * 3600 + parseInt(parts[1]) * 60 + parseInt(parts[2]);
  if (parts.length === 2) return parseInt(parts[0]) * 60 + parseInt(parts[1]);
  return 0;
}

export function fmtDuration(secs: number): string {
  if (secs < 60) return `${secs}s`;
  if (secs < 3600) return `${Math.floor(secs / 60)}m${secs % 60}s`;
  return `${Math.floor(secs / 3600)}h${Math.floor((secs % 3600) / 60)}m`;
}

export function splitGraph(
  graph: WorkflowGraph,
  filter: (node: GraphNode) => boolean
): WorkflowGraph {
  const nodeIds = new Set(graph.nodes.filter(filter).map((n) => n.id));
  return {
    session_label: graph.session_label,
    nodes: graph.nodes.filter((n) => nodeIds.has(n.id)),
    edges: graph.edges.filter((e) => nodeIds.has(e.source) && nodeIds.has(e.target)),
  };
}

export function computeLayout(graph: WorkflowGraph): LayoutResult {
  const nodeMap = new Map<number, GraphNode>();
  for (const n of graph.nodes) nodeMap.set(n.id, n);

  const indegree = new Map<number, number>();
  for (const n of graph.nodes) indegree.set(n.id, 0);
  for (const e of graph.edges) indegree.set(e.target, (indegree.get(e.target) || 0) + 1);

  const layers: number[][] = [];
  const nodeLayer = new Map<number, number>();
  let currentLayer = graph.nodes.filter((n) => (indegree.get(n.id) || 0) === 0).map((n) => n.id);

  while (currentLayer.length > 0) {
    layers.push([...currentLayer]);
    for (const id of currentLayer) nodeLayer.set(id, layers.length - 1);
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

  const nodeDurations = new Map<number, string>();
  for (const node of graph.nodes) {
    const outEdge = graph.edges
      .filter((e) => e.source === node.id)
      .sort((a, b) => timeToSecs(a.timestamp) - timeToSecs(b.timestamp))[0];
    if (outEdge && node.created_at) {
      const startSecs = timeToSecs(node.created_at);
      const endSecs = timeToSecs(outEdge.timestamp);
      if (endSecs > startSecs) nodeDurations.set(node.id, fmtDuration(endSecs - startSecs));
    }
  }

  let totalDuration = '';
  if (graph.edges.length > 0) {
    const allTimes = graph.edges.map((e) => timeToSecs(e.timestamp)).sort((a, b) => a - b);
    const diff = allTimes[allTimes.length - 1] - allTimes[0];
    if (diff > 0) totalDuration = fmtDuration(diff);
  }

  const maxLayerWidth = Math.max(...layers.map((l) => l.length), 1);
  const svgW = PADDING * 2 + maxLayerWidth * (NODE_W + H_GAP) - H_GAP;
  const svgH = PADDING * 2 + layers.length * (NODE_H + V_GAP) - V_GAP;

  return { layoutNodes, edges: layoutEdges, svgW, svgH, totalDuration, nodeDurations };
}
