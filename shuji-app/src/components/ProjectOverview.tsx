import { useEffect, useState } from 'react';
import { listShujiTree, generateDeliveryReport } from '../api';
import type { ShujiEntry } from '../api';
import type { PlanInfo, Project } from '../types';
import { getDeptMeta } from '../constants';
import { formatError } from '../utils/error';
import { Card } from './ui/Card';

interface ProjectOverviewProps {
  project: Project | null;
  activeDepts: string[];
  planInfo: PlanInfo | null;
  onOpenProject: () => void;
  onDocSelect?: (path: string) => void;
}

export default function ProjectOverview({
  project,
  activeDepts,
  planInfo,
  onOpenProject,
  onDocSelect,
}: ProjectOverviewProps) {
  const [latestDocs, setLatestDocs] = useState<ShujiEntry[]>([]);
  const [docsLoading, setDocsLoading] = useState(false);
  const [error, setError] = useState('');
  const [report, setReport] = useState<string | null>(null);
  const [reportLoading, setReportLoading] = useState(false);

  const handleGenerateReport = async () => {
    setReportLoading(true);
    try {
      const r = await generateDeliveryReport();
      setReport(r);
    } catch (e) {
      setError(formatError(e));
    } finally {
      setReportLoading(false);
    }
  };

  useEffect(() => {
    if (!project?.working_dir) return;
    setDocsLoading(true);
    setError('');
    listShujiTree(project.working_dir)
      .then((tree) => {
        setLatestDocs(
          flatten(tree)
            .filter((entry) => entry.path.startsWith('.shuji/') && entry.name.endsWith('.md'))
            .slice(0, 5)
        );
        setDocsLoading(false);
      })
      .catch((e) => {
        setError(formatError(e));
        setLatestDocs([]);
        setDocsLoading(false);
      });
  }, [project?.working_dir]);

  if (!project) {
    return (
      <div className="h-full flex items-center justify-center surface-paper">
        <div className="text-center max-w-md">
          <p className="text-ink-600 text-body mb-2">尚未开卷</p>
          <p className="text-ui text-ink-500 mb-6 leading-relaxed">
            体验枢机：打开工作目录，拟旨下诏，驱动各部门协同运作。
          </p>
          <button
            onClick={onOpenProject}
            className="px-5 py-2 bg-ink-900 text-ink-50 text-ui rounded-lg hover:bg-ink-800 transition-colors"
          >
            打开项目
          </button>
        </div>
      </div>
    );
  }

  const done = planInfo?.batches.filter((b) => b.status === 'done').length || 0;
  const total = planInfo?.batches.length || 0;

  return (
    <div className="h-full overflow-y-auto surface-paper p-8">
      <Card variant="paper" className="max-w-3xl mx-auto p-6">
        <div className="font-display text-display font-bold text-ink-900 mb-1">{project.name}</div>
        <p className="text-caption text-ink-400 font-mono truncate mb-6">{project.working_dir}</p>

        <section className="mb-6">
          <h3 className="font-display text-ui text-ink-600 font-semibold title-rule-gold mb-3">
            当值诸司
          </h3>
          {activeDepts.length === 0 ? (
            <p className="text-body text-ink-400">暂无活跃部门</p>
          ) : (
            <div className="flex flex-wrap gap-2">
              {activeDepts.map((dept) => (
                <span
                  key={dept}
                  className="px-2 py-1 rounded-full bg-ink-100 text-ui text-ink-700 flex items-center gap-1"
                >
                  <span
                    className="animate-pulse"
                    style={{ color: getDeptMeta(dept)?.color || '#6b7280' }}
                  >
                    ●
                  </span>
                  {dept}
                </span>
              ))}
            </div>
          )}
        </section>

        <section className="mb-6">
          <h3 className="font-display text-ui text-ink-600 font-semibold title-rule-gold mb-3">
            最新牍文
          </h3>
          {docsLoading ? (
            <p className="text-body text-ink-400">开卷中…</p>
          ) : error ? (
            <p className="text-body text-vermillion">{error}</p>
          ) : latestDocs.length === 0 ? (
            <p className="text-body text-ink-400">架阁尚无新牍</p>
          ) : (
            <div className="space-y-1">
              {latestDocs.map((doc) => (
                <div
                  key={doc.path}
                  onClick={() => onDocSelect?.(doc.path)}
                  className="flex items-center gap-2 text-body cursor-pointer hover:bg-ink-100/50 rounded px-1 -mx-1 transition-colors"
                >
                  <span className="font-mono text-ink-800">{doc.name.replace(/\.md$/, '')}</span>
                  <span className="text-ink-300">·</span>
                  <span className="text-ink-500">{doc.type_label}</span>
                </div>
              ))}
            </div>
          )}
        </section>

        {planInfo && total > 0 && (
          <section>
            <h3 className="font-display text-ui text-ink-600 font-semibold title-rule-gold mb-3">
              工部计划: {done}/{total}
            </h3>
            <div className="w-full h-2 bg-ink-200 rounded-full overflow-hidden mb-2">
              <div
                className="h-full bg-gold"
                style={{ width: `${Math.round((done / total) * 100)}%` }}
              />
            </div>
            <div className="grid grid-cols-1 sm:grid-cols-2 gap-1">
              {planInfo.batches.map((b, i) => (
                <div
                  key={i}
                  className={`text-ui ${b.status === 'current' ? 'text-ink-900 font-medium' : 'text-ink-500'}`}
                >
                  · {b.name}
                </div>
              ))}
            </div>
          </section>
        )}

        <section className="mt-6 pt-4 border-t border-fold">
          <button
            onClick={handleGenerateReport}
            disabled={reportLoading}
            className="px-4 py-2 bg-ink-900 text-ink-50 text-ui rounded-lg hover:bg-ink-800 transition-colors disabled:opacity-50"
          >
            {reportLoading ? '生成中…' : '生成交付报告'}
          </button>
          {report && (
            <div className="mt-3 p-3 rounded-lg bg-ink-100/50 border border-fold text-caption font-mono whitespace-pre-wrap text-ink-700 max-h-64 overflow-y-auto">
              {report}
            </div>
          )}
        </section>
      </Card>
    </div>
  );
}

function flatten(entries: ShujiEntry[]): ShujiEntry[] {
  return entries.flatMap((entry) => (entry.is_dir ? flatten(entry.children) : [entry]));
}
