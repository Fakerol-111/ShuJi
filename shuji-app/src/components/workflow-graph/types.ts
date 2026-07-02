import type { GraphNode, GraphEdge } from '../../types';

export interface LayoutNode extends GraphNode {
  layer: number;
  index: number;
  x: number;
  y: number;
}

export interface LayoutEdge extends GraphEdge {
  src: LayoutNode;
  dst: LayoutNode;
}

export interface ArchiveEntry {
  filename: string;
  label: string;
  ts: string;
}

export interface LayoutResult {
  layoutNodes: LayoutNode[];
  edges: LayoutEdge[];
  svgW: number;
  svgH: number;
  totalDuration: string;
  nodeDurations: Map<number, string>;
}
