import { useEffect, useState } from 'react';
import { listShujiTree } from '../api';
import type { ShujiEntry } from '../api';
import type { Project } from '../types';

interface AgentIdleStateProps {
  project: Project | null;
  onDocSelect?: (path: string) => void;
  onOpenProject?: () => void;
}

export default function AgentIdleState({
  project,
  onDocSelect,
  onOpenProject,
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
      <div className="max-w-lg mx-auto space-y-4">
        <div>
          <h2 className="font-display text-lg font-bold text-ink-900">{project.name}</h2>
          <p className="text-caption text-ink-400 font-mono truncate mt-1">{project.working_dir}</p>
        </div>

        {project.goal && <p className="text-body text-ink-600 leading-relaxed">{project.goal}</p>}

        {loading ? (
          <p className="text-caption text-ink-400">开卷中…</p>
        ) : latestDocs.length > 0 ? (
          <div>
            <h3 className="text-ui font-semibold text-ink-700 mb-2">最近文档</h3>
            <div className="space-y-1">
              {latestDocs.map((doc) => (
                <div
                  key={doc.path}
                  onClick={() => onDocSelect?.(doc.path)}
                  className="flex items-center gap-2 text-body cursor-pointer hover:bg-ink-100/50 rounded px-2 -mx-2 py-1 transition-colors"
                >
                  <span className="font-mono text-ink-800">{doc.name.replace(/\.md$/, '')}</span>
                  <span className="text-ink-300">·</span>
                  <span className="text-ink-500">{doc.type_label}</span>
                </div>
              ))}
            </div>
          </div>
        ) : null}

        <div className="border-t border-fold pt-4">
          <p className="text-caption text-ink-500 leading-relaxed">
            敕令示例：
            <br />
            「分析代码库，了解项目结构」
            <br />
            「实现用户注册功能」
          </p>
        </div>
      </div>
    </div>
  );
}

function flatten(entries: ShujiEntry[]): ShujiEntry[] {
  return entries.flatMap((entry) => (entry.is_dir ? flatten(entry.children) : [entry]));
}
