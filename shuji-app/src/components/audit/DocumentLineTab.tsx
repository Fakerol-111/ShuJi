import type { DocumentLineTabProps } from './types';
import { NODE_KIND_COLORS } from './types';

export default function DocumentLineTab({
  t,
  docLineRuns,
  docLineRunId,
  onChangeDocLineRunId,
  docLineDocId,
  onChangeDocLineDocId,
  docLine,
  docLineLoading,
  onLoadDocLine,
  onDocLineNodeClick,
}: DocumentLineTabProps) {
  return (
    <div className="p-3 space-y-2 flex-1 overflow-y-auto min-h-0">
      <div className="flex flex-wrap gap-1 items-center">
        {docLineRuns.length > 0 && (
          <select
            value={docLineRunId}
            onChange={(e) => onChangeDocLineRunId(e.target.value)}
            className="px-2 py-1 text-caption rounded bg-ink-100 border border-fold text-ink-700 outline-none max-w-[140px]"
          >
            {docLineRuns.map((r) => (
              <option key={r} value={r}>
                {r}
              </option>
            ))}
          </select>
        )}
        <button
          onClick={() => onLoadDocLine(docLineRunId)}
          className="px-2 py-1 rounded bg-ink-700 text-white text-caption hover:bg-ink-600"
        >
          加载任务线
        </button>
        <input
          type="text"
          placeholder="按文档 ID 定位"
          value={docLineDocId}
          onChange={(e) => onChangeDocLineDocId(e.target.value)}
          onKeyDown={(e) => e.key === 'Enter' && onLoadDocLine(undefined, docLineDocId)}
          className="flex-1 min-w-[100px] px-2 py-1 text-caption rounded bg-ink-100 border border-fold text-ink-700 placeholder-ink-300 outline-none"
        />
        <button
          onClick={() => onLoadDocLine(undefined, docLineDocId)}
          className="px-2 py-1 rounded bg-ink-700 text-white text-caption hover:bg-ink-600"
        >
          定位
        </button>
      </div>
      {docLineLoading && <div className="text-caption text-ink-400">{t('common.loading')}</div>}
      {!docLineLoading && !docLine && (
        <div className="text-caption text-ink-300">选择 run 或输入文档 ID 查看端到端证据链</div>
      )}
      {docLine && (
        <div className="space-y-2">
          <div className="text-caption text-ink-600">
            <span className="font-mono">{docLine.run_id}</span>
            <span className="mx-1">·</span>
            <span>{docLine.status}</span>
            {docLine.session_label && (
              <span className="text-ink-400 ml-1">— {docLine.session_label}</span>
            )}
          </div>
          <div className="space-y-1">
            {docLine.nodes.map((node) => (
              <div
                key={node.node_id}
                className={`rounded border p-2 cursor-pointer transition-colors hover:opacity-90 ${
                  NODE_KIND_COLORS[node.kind] || 'border-fold bg-ink-50'
                } ${node.highlight ? 'ring-2 ring-gold/50' : ''} ${node.stale ? 'opacity-80' : ''}`}
                onClick={() => onDocLineNodeClick(node)}
              >
                <div className="flex items-center gap-1.5 text-caption flex-wrap">
                  <span className="text-[9px] uppercase text-ink-400">{node.kind}</span>
                  <span className="font-mono text-ink-700">{node.label}</span>
                  {node.status && node.status !== '-' && (
                    <span className="px-1 rounded text-[9px] bg-ink-100 text-ink-500">
                      {node.status}
                    </span>
                  )}
                  {node.stale && (
                    <span className="px-1 rounded text-[9px] bg-vermillion/10 text-vermillion">
                      stale
                    </span>
                  )}
                  {node.role && <span className="text-[9px] text-ink-400">{node.role}</span>}
                </div>
                {node.timestamp && (
                  <div className="text-[9px] text-ink-300 font-mono mt-0.5">{node.timestamp}</div>
                )}
              </div>
            ))}
          </div>
          {docLine.edges.length > 0 && (
            <div className="rounded border border-fold p-2 space-y-0.5">
              <div className="text-caption font-semibold text-ink-700 mb-1">关系</div>
              {docLine.edges.slice(0, 24).map((edge, i) => (
                <div key={i} className="text-[10px] font-mono text-ink-500 truncate">
                  {edge.from.split(':').pop()} —{edge.relation}→ {edge.to.split(':').pop()}
                </div>
              ))}
              {docLine.edges.length > 24 && (
                <div className="text-[9px] text-ink-300">…共 {docLine.edges.length} 条边</div>
              )}
            </div>
          )}
        </div>
      )}
    </div>
  );
}
