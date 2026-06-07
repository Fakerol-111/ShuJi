import { useEffect, useRef, useState } from "react";
import { getWorkflowGraph } from "../api";
import type { WorkflowGraph, GraphNode, GraphEdge } from "../types";
import { DEPT_META } from "../constants";

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

export default function WorkflowGraphView() {
  const [graph, setGraph] = useState<WorkflowGraph | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState("");
  const svgRef = useRef<SVGSVGElement>(null);

  const fetchGraph = () => {
    setLoading(true);
    setError("");
    getWorkflowGraph()
      .then((g) => { setGraph(g); setLoading(false); })
      .catch((e) => { setError(String(e)); setLoading(false); });
  };

  useEffect(() => {
    fetchGraph();
    const timer = setInterval(fetchGraph, 5000); // 每 5s 自动刷新
    return () => clearInterval(timer);
  }, []);

  if (loading && !graph) {
    return (
      <div className="h-full flex items-center justify-center text-ink-400">
        加载文移图中…
      </div>
    );
  }

  if (error) {
    return (
      <div className="h-full flex flex-col items-center justify-center gap-3">
        <p className="text-vermillion">{error}</p>
        <button onClick={fetchGraph} className="text-ui text-gold underline">重试</button>
      </div>
    );
  }

  if (!graph || graph.nodes.length === 0) {
    return (
      <div className="h-full flex items-center justify-center text-ink-400">
        暂无文移记录
      </div>
    );
  }

  // ── Layered layout ──────────────────────────────────
  const nodeMap = new Map<number, GraphNode>();
  for (const n of graph.nodes) nodeMap.set(n.id, n);

  // indegree per node
  const indegree = new Map<number, number>();
  for (const n of graph.nodes) indegree.set(n.id, 0);
  for (const e of graph.edges) {
    indegree.set(e.target, (indegree.get(e.target) || 0) + 1);
  }

  // BFS layer assignment
  const layers: number[][] = []; // layer index → node ids
  const nodeLayer = new Map<number, number>();

  // Start nodes = indegree 0 (内阁#1 always)
  let currentLayer = graph.nodes
    .filter((n) => (indegree.get(n.id) || 0) === 0)
    .map((n) => n.id);

  while (currentLayer.length > 0) {
    layers.push([...currentLayer]);
    const nextLayer: number[] = [];
    for (const id of currentLayer) {
      nodeLayer.set(id, layers.length - 1);
    }
    for (const e of graph.edges) {
      if (currentLayer.includes(e.source) && !nodeLayer.has(e.target)) {
        if (!nextLayer.includes(e.target)) nextLayer.push(e.target);
      }
    }
    // also add any node whose ALL predecessors are assigned
    for (const n of graph.nodes) {
      if (nodeLayer.has(n.id)) continue;
      const preds = graph.edges.filter((e) => e.target === n.id);
      if (preds.length > 0 && preds.every((p) => nodeLayer.has(p.source))) {
        if (!nextLayer.includes(n.id)) nextLayer.push(n.id);
      }
    }
    currentLayer = nextLayer;
  }

  // Assign any unassigned nodes to last layer
  for (const n of graph.nodes) {
    if (!nodeLayer.has(n.id)) {
      nodeLayer.set(n.id, layers.length - 1);
      if (layers.length === 0) layers.push([]);
      layers[layers.length - 1].push(n.id);
    }
  }

  // Build layout nodes
  const layoutNodes: LayoutNode[] = [];
  for (let li = 0; li < layers.length; li++) {
    const ids = layers[li].sort((a, b) => a - b); // sort by creation order
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

  // Build layout edges
  const layoutEdges: LayoutEdge[] = [];
  for (const e of graph.edges) {
    const src = lnMap.get(e.source);
    const dst = lnMap.get(e.target);
    if (src && dst) layoutEdges.push({ ...e, src, dst });
  }

  // SVG dimensions
  const maxLayerWidth = Math.max(...layers.map((l) => l.length));
  const svgW = PADDING * 2 + maxLayerWidth * (NODE_W + H_GAP) - H_GAP;
  const svgH = PADDING * 2 + layers.length * (NODE_H + V_GAP) - V_GAP;

  return (
    <div className="h-full overflow-auto bg-surface-paper">
      <div className="flex items-center gap-2 px-4 py-2 border-b border-fold shrink-0">
        <h2 className="font-display text-ui font-semibold text-ink-800">文移图</h2>
        <span className="text-caption text-ink-500">
          {graph.nodes.length} 节点 · {graph.edges.length} 边
        </span>
        <button onClick={fetchGraph} className="ml-auto text-caption text-gold hover:text-gold/80">
          刷新
        </button>
      </div>
      <svg
        ref={svgRef}
        viewBox={`0 0 ${svgW} ${svgH}`}
        className="min-w-full"
        style={{ width: svgW, height: svgH }}
      >
        <defs>
          <marker id="arrowhead" markerWidth="10" markerHeight="7" refX="10" refY="3.5" orient="auto">
            <polygon points="0 0, 10 3.5, 0 7" fill="#8B7355" />
          </marker>
        </defs>

        {/* Edges */}
        {layoutEdges.map((e) => {
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
              {/* Task label on edge */}
              <text
                x={(sx + dx) / 2}
                y={cy - 6}
                textAnchor="middle"
                className="text-caption"
                fill="#8B7355"
                fontSize="11"
              >
                {e.task_id.length > 20 ? e.task_id.slice(0, 18) + "…" : e.task_id}
              </text>
            </g>
          );
        })}

        {/* Nodes */}
        {layoutNodes.map((n) => {
          const meta = DEPT_META[n.role] || { color: "#6b7280", label: n.role };
          const isMulti = n.instance > 1;
          return (
            <g key={`node-${n.id}`}>
              {/* Node body */}
              <rect
                x={n.x}
                y={n.y}
                width={NODE_W}
                height={NODE_H}
                rx="8"
                ry="8"
                fill={n.status === "failed" ? "#FDE8EC" : "#F5F0E8"}
                stroke={n.status === "failed" ? "#C41E3A" : n.status === "completed" ? "#2D5A3F" : meta.color}
                strokeWidth={n.status === "active" ? 2 : 1}
                opacity={n.status === "completed" ? 0.7 : 1}
              />
              {/* Role name */}
              <text
                x={n.x + NODE_W / 2}
                y={n.y + 22}
                textAnchor="middle"
                className="font-display"
                fill="#1A1512"
                fontSize="13"
                fontWeight="600"
              >
                {n.role}{isMulti ? `#${n.instance}` : ""}
              </text>
              {/* Task summary */}
              <text
                x={n.x + NODE_W / 2}
                y={n.y + 42}
                textAnchor="middle"
                fill="#5C4F3E"
                fontSize="10"
              >
                {n.task_summary.length > 18 ? n.task_summary.slice(0, 16) + "…" : n.task_summary}
              </text>
              {/* Status dot */}
              <circle
                cx={n.x + 12}
                cy={n.y + 12}
                r="4"
                fill={n.status === "active" ? "#B8860B" : n.status === "completed" ? "#2D5A3F" : "#C41E3A"}
              />
            </g>
          );
        })}
      </svg>
    </div>
  );
}
