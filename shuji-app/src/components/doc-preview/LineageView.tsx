import type { LineageNode } from '../../types';

export default function LineageView({ node, depth }: { node: LineageNode; depth: number }) {
  const statusColor =
    node.status === 'in_review'
      ? 'text-vermillion'
      : node.status === 'approved'
        ? 'text-jade'
        : node.status === 'rejected'
          ? 'text-vermillion/60'
          : 'text-ink-500';

  return (
    <div className="font-mono text-caption">
      <div className="flex items-center gap-2 py-1" style={{ paddingLeft: `${depth * 20}px` }}>
        {depth > 0 && <span className="text-ink-300 shrink-0">└─</span>}
        <span className="font-bold text-ink-800">{node.id}</span>
        <span className="text-ink-400">({node.doc_type})</span>
        <span className="text-ink-400">— {node.author}</span>
        {node.status && <span className={statusColor}>{node.status}</span>}
      </div>
      <div className="text-[9px] text-ink-400" style={{ paddingLeft: `${depth * 20 + 16}px` }}>
        {node.timestamp}
        {node.refs.length > 0 && ` · 引用: [${node.refs.join(', ')}]`}
      </div>
      {node.children.map((child) => (
        <LineageView key={child.id} node={child} depth={depth + 1} />
      ))}
    </div>
  );
}
