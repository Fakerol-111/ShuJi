import { getDeptMeta } from '../../constants';
import { NODE_W, NODE_H } from './layout';
import type { LayoutResult } from './types';

const UPPER_MARKER = 'arrowhead';
const LOWER_MARKER = 'arrowhead-lower';

export default function GraphSection({
  layout,
  isLower,
  t,
}: {
  layout: LayoutResult | null;
  isLower: boolean;
  t: (key: string) => string;
}) {
  const markerId = isLower ? LOWER_MARKER : UPPER_MARKER;

  if (!layout) return null;

  return (
    <svg
      viewBox={`0 0 ${layout.svgW} ${layout.svgH}`}
      className="min-w-full"
      style={{ width: layout.svgW, height: layout.svgH }}
    >
      <defs>
        <marker id={markerId} markerWidth="10" markerHeight="7" refX="10" refY="3.5" orient="auto">
          <polygon points="0 0, 10 3.5, 0 7" fill={isLower ? '#B83A3A' : '#8B7355'} />
        </marker>
      </defs>
      {layout.edges.map((e) => {
        const sx = e.src.x + NODE_W / 2;
        const sy = e.src.y + NODE_H;
        const dx = e.dst.x + NODE_W / 2;
        const dy = e.dst.y;
        const cy = (sy + dy) / 2;
        return (
          <g key={`${isLower ? 'l' : ''}edge-${e.id}`}>
            <path
              d={`M ${sx} ${sy} C ${sx} ${cy}, ${dx} ${cy}, ${dx} ${dy}`}
              fill="none"
              stroke={isLower ? '#B83A3A' : '#C4B8A2'}
              strokeWidth="2"
              markerEnd={`url(#${markerId})`}
            />
            <text
              x={(sx + dx) / 2}
              y={cy - 6}
              textAnchor="middle"
              fill={isLower ? '#B83A3A' : '#8B7355'}
              fontSize="11"
            >
              {e.task_id.length > 24 ? e.task_id.slice(0, 22) + '…' : e.task_id}
            </text>
          </g>
        );
      })}
      {layout.layoutNodes.map((n) => {
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
        const durStr = layout.nodeDurations.get(n.id);
        return (
          <g key={`${isLower ? 'l' : ''}node-${n.id}`} className="group">
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
                n.status === 'failed' ? '#FDE8EC' : n.status === 'planned' ? '#FAFAF5' : '#F5F0E8'
              }
              stroke={color}
              strokeWidth={n.status === 'active' ? 2 : 1}
              strokeDasharray={n.status === 'planned' ? '5,3' : 'none'}
              opacity={n.status === 'completed' ? 0.7 : n.status === 'planned' ? 0.6 : 1}
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
              {n.task_summary.length > 18 ? n.task_summary.slice(0, 16) + '…' : n.task_summary}
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
              {durStr ? ` · ${durStr}` : ''}
            </text>
          </g>
        );
      })}
    </svg>
  );
}
