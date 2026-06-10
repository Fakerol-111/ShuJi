import { Card } from './ui/Card';
import { Button } from './ui/Button';

interface Props {
  summary: { elapsed: string; tokens: number; cached: number; uncached: number };
  onOpenProject: () => void;
}

export default function DemoSummaryCard({ summary, onOpenProject }: Props) {
  const cacheRate =
    summary.tokens > 0
      ? Math.round((summary.cached / (summary.cached + summary.uncached)) * 100)
      : null;
  return (
    <div className="h-full overflow-y-auto surface-paper p-8">
      <Card variant="paper" className="max-w-3xl mx-auto p-6">
        <div className="text-center mb-2">
          <div className="w-12 h-12 mx-auto mb-3 rounded-full bg-jade-light flex items-center justify-center">
            <svg
              className="w-6 h-6 text-jade"
              fill="none"
              viewBox="0 0 24 24"
              stroke="currentColor"
              strokeWidth={2.5}
            >
              <path strokeLinecap="round" strokeLinejoin="round" d="M4.5 12.75l6 6 9-13.5" />
            </svg>
          </div>
          <h2 className="font-display text-display font-bold text-ink-900">Demo 完成</h2>
          <p className="text-body text-ink-600 mt-1">体验流程已结束，以下是本次 Demo 的概览。</p>
        </div>
        <section className="mb-6">
          <h3 className="font-display text-ui text-ink-600 font-semibold title-rule-gold mb-3">
            汇总
          </h3>
          <div className="grid grid-cols-1 sm:grid-cols-3 gap-3">
            <div className="bg-surface-parchment border border-fold rounded-lg p-3 text-center">
              <p className="text-caption text-ink-500 mb-1">耗时</p>
              <p className="font-display text-xl text-ink-900 font-bold">{summary.elapsed}</p>
            </div>
            <div className="bg-surface-parchment border border-fold rounded-lg p-3 text-center">
              <p className="text-caption text-ink-500 mb-1">Token 消耗</p>
              <p className="font-display text-xl text-ink-900 font-bold">
                {summary.tokens.toLocaleString()}
              </p>
              <p className="text-caption text-ink-500 mt-1">
                缓存 {summary.cached.toLocaleString()} / 未缓存 {summary.uncached.toLocaleString()}
              </p>
            </div>
            <div className="bg-surface-parchment border border-fold rounded-lg p-3 text-center">
              <p className="text-caption text-ink-500 mb-1">缓存命中率</p>
              <p className="font-display text-xl text-ink-900 font-bold">
                {cacheRate !== null ? `${cacheRate}%` : 'N/A'}
              </p>
            </div>
          </div>
        </section>
        <section className="mb-6">
          <h3 className="font-display text-ui text-ink-600 font-semibold title-rule-gold mb-3">
            下一步
          </h3>
          <ul className="space-y-2 text-body text-ink-700">
            <li className="leading-relaxed">
              <strong>打开真实项目</strong> — 选择您的项目目录，枢机将根据需求自动规划并执行任务。
            </li>
            <li className="leading-relaxed">
              <strong>调整参与模式</strong> — 使用{' '}
              <code className="text-vermillion bg-vermillion-light px-1 rounded text-ui">
                /level-2
              </code>{' '}
              切换审批模式，让系统在关键节点等待您的确认。
            </li>
          </ul>
        </section>
        <div className="flex justify-center gap-3">
          <Button variant="secondary" onClick={onOpenProject}>
            打开真实项目
          </Button>
        </div>
      </Card>
    </div>
  );
}
