import { useEffect, useState } from 'react';
import { listShujiTree } from '../api';
import { SealLogo } from './SealLogo';
import type { ShujiEntry } from '../api';
import type { Project } from '../types';

const EXAMPLES = [
  '分析代码库，了解项目结构',
  '实现用户注册功能',
  '修复登录页表单校验问题',
];

const DOC_TYPE_COLORS: Record<string, string> = {
  dsgn: '#3D6B8E',
  plan: '#b8860b',
  revw: '#2E7D8C',
  task: '#B45309',
  ctrt: '#B83A3A',
  rprt: '#A16207',
};

interface AgentIdleStateProps {
  project: Project | null;
  onDocSelect?: (path: string) => void;
  onOpenProject?: () => void;
  onFillInput?: (text: string) => void;
}

export default function AgentIdleState({
  project,
  onDocSelect,
  onOpenProject,
  onFillInput,
}: AgentIdleStateProps) {
  const [latestDocs, setLatestDocs] = useState<ShujiEntry[]>([]);
  const [loading, setLoading] = useState(false);

  useEffect(() => {
    if (!project?.working_dir) return;
    setLoading(true);
    listShujiTree(project.working_dir)
      .then((tree) => {
        setLatestDocs(
          flatten(tree)
            .filter((entry) => entry.path.startsWith('.shuji/') && entry.name.endsWith('.md'))
            .slice(0, 3)
        );
        setLoading(false);
      })
      .catch(() => setLatestDocs([]));
  }, [project?.working_dir]);

  if (!project) {
    return (
      <div className="flex-1 flex items-center justify-center p-6">
        <div className="text-center max-w-md">
          <p className="text-ink-600 text-body mb-2">尚未开卷</p>
          <p className="text-ui text-ink-500 mb-6 leading-relaxed">
            体验枢机：打开工作目录，拟旨下诏，驱动各部门协同运作。
          </p>
          {onOpenProject && (
            <button
              onClick={onOpenProject}
              className="px-5 py-2 bg-ink-900 text-ink-50 text-ui rounded-lg hover:bg-ink-800 transition-colors"
            >
              打开项目
            </button>
          )}
        </div>
      </div>
    );
  }

  return (
    <div className="flex-1 overflow-y-auto p-6">
      <div className="max-w-lg mx-auto space-y-5">
        {/* Seal + title */}
        <div className="text-center">
          <div className="inline-flex mb-3">
            <SealLogo size={48} />
          </div>
          <h2 className="font-display text-title font-bold text-ink-900">{project.name}</h2>
          <p className="font-mono text-caption text-ink-400 truncate mt-1">{project.working_dir}</p>
        </div>

        {project.goal && (
          <p className="text-body text-ink-600 leading-relaxed text-center">{project.goal}</p>
        )}

        {/* 敕令示例 */}
        <div>
          <h3 className="text-ui font-semibold text-ink-700 mb-2 title-rule-gold">敕令示例</h3>
          <div className="flex flex-col gap-1.5">
            {EXAMPLES.map((text) => (
              <button
                key={text}
                onClick={() => onFillInput?.(text)}
                className="flex items-center gap-2 px-3 py-2 rounded-lg border border-fold bg-surface-elevated hover:border-gold/40 hover:bg-surface-parchment/60 transition-colors cursor-pointer text-left"
              >
                <SealLogo size={16} />
                <span className="text-ui text-ink-700">{text}</span>
              </button>
            ))}
          </div>
        </div>

        {/* 最近牍章 */}
        {!loading && latestDocs.length > 0 && (
          <div>
            <h3 className="text-ui font-semibold text-ink-700 mb-2 title-rule-gold">最近牍章</h3>
            <div className="flex flex-wrap gap-2">
              {latestDocs.map((doc) => {
                const typeColor = DOC_TYPE_COLORS[doc.type_label] || '#8B7355';
                return (
                  <div
                    key={doc.path}
                    onClick={() => onDocSelect?.(doc.path)}
                    className="flex items-center gap-1.5 px-2 py-1 rounded border border-fold bg-surface-elevated cursor-pointer hover:border-gold/40 transition-colors"
                    style={{ borderLeftColor: typeColor, borderLeftWidth: 3 }}
                  >
                    <span className="text-ui text-ink-700">{doc.name.replace(/\.md$/, '')}</span>
                    <span
                      className="text-caption px-1 rounded font-medium"
                      style={{ backgroundColor: `${typeColor}18`, color: typeColor }}
                    >
                      {doc.type_label}
                    </span>
                  </div>
                );
              })}
            </div>
          </div>
        )}

        {loading && (
          <p className="text-caption text-ink-400 text-center">开卷中…</p>
        )}
      </div>
    </div>
  );
}

function flatten(entries: ShujiEntry[]): ShujiEntry[] {
  return entries.flatMap((entry) => (entry.is_dir ? flatten(entry.children) : [entry]));
}
