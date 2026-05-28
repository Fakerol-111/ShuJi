import { useEffect, useState } from "react";
import { listShujiTree } from "../api";
import type { ShujiEntry } from "../api";
import type { PlanInfo, Project } from "../types";
import { DEPT_COLORS } from "./DeptStatusBar";

interface ProjectOverviewProps {
  project: Project | null;
  activeDepts: string[];
  planInfo: PlanInfo | null;
  onOpenProject: () => void;
}

export default function ProjectOverview({ project, activeDepts, planInfo, onOpenProject }: ProjectOverviewProps) {
  const [latestDocs, setLatestDocs] = useState<ShujiEntry[]>([]);
  const [docsLoading, setDocsLoading] = useState(false);
  const [error, setError] = useState("");

  useEffect(() => {
    if (!project?.working_dir) return;
    setDocsLoading(true);
    setError("");
    listShujiTree(project.working_dir).then((tree) => {
      setLatestDocs(flatten(tree).filter((entry) => entry.path.startsWith(".shuji/") && entry.name.endsWith(".md")).slice(0, 5));
      setDocsLoading(false);
    }).catch((e) => {
      console.error("文档树加载失败:", e);
      setError("加载失败");
      setLatestDocs([]);
      setDocsLoading(false);
    });
  }, [project?.working_dir]);

  if (!project) {
    return (
      <div className="h-full flex items-center justify-center bg-ink-50">
        <div className="text-center">
          <p className="text-ink-500 text-sm mb-3">尚未加载项目</p>
          <button onClick={onOpenProject} className="px-4 py-2 bg-ink-900 text-ink-50 text-sm rounded-lg hover:bg-ink-800 transition-colors">
            打开项目
          </button>
        </div>
      </div>
    );
  }

  const done = planInfo?.batches.filter((b) => b.status === "done").length || 0;
  const total = planInfo?.batches.length || 0;

  return (
    <div className="h-full overflow-y-auto bg-ink-50 p-8">
      <div className="max-w-3xl mx-auto bg-white border border-ink-200 rounded-2xl shadow-sm p-6">
        <div className="text-xs text-ink-400 mb-1">项目</div>
        <h2 className="text-2xl font-bold text-ink-900 mb-1">{project.name}</h2>
        <p className="text-xs text-ink-400 font-mono truncate mb-6">{project.working_dir}</p>

        <section className="mb-6">
          <h3 className="text-xs font-bold text-ink-500 mb-2">当前活跃部门</h3>
          {activeDepts.length === 0 ? (
            <p className="text-sm text-ink-400">暂无活跃部门</p>
          ) : (
            <div className="flex flex-wrap gap-2">
              {activeDepts.map((dept) => (
                <span key={dept} className="px-2 py-1 rounded-full bg-ink-100 text-xs text-ink-700 flex items-center gap-1">
                  <span className="animate-pulse" style={{ color: DEPT_COLORS[dept] || "#6b7280" }}>●</span>{dept}
                </span>
              ))}
            </div>
          )}
        </section>

        <section className="mb-6">
          <h3 className="text-xs font-bold text-ink-500 mb-2">最新产出</h3>
          {docsLoading ? (
            <p className="text-sm text-ink-400">加载中...</p>
          ) : error ? (
            <p className="text-sm text-red-500">{error}</p>
          ) : latestDocs.length === 0 ? (
            <p className="text-sm text-ink-400">暂无文档产出</p>
          ) : (
            <div className="space-y-1">
              {latestDocs.map((doc) => (
                <div key={doc.path} className="flex items-center gap-2 text-sm">
                  <span className="font-mono text-ink-800">{doc.name.replace(/\.md$/, "")}</span>
                  <span className="text-ink-300">·</span>
                  <span className="text-ink-500">{doc.type_label}</span>
                </div>
              ))}
            </div>
          )}
        </section>

        {planInfo && total > 0 && (
          <section>
            <h3 className="text-xs font-bold text-ink-500 mb-2">工部计划: {done}/{total}</h3>
            <div className="w-full h-2 bg-ink-200 rounded-full overflow-hidden mb-2">
              <div className="h-full bg-amber-500" style={{ width: `${Math.round((done / total) * 100)}%` }} />
            </div>
            <div className="grid grid-cols-1 sm:grid-cols-2 gap-1">
              {planInfo.batches.map((b, i) => (
                <div key={i} className={`text-xs ${b.status === "current" ? "text-ink-900 font-medium" : "text-ink-500"}`}>· {b.name}</div>
              ))}
            </div>
          </section>
        )}
      </div>
    </div>
  );
}

function flatten(entries: ShujiEntry[]): ShujiEntry[] {
  return entries.flatMap((entry) => entry.is_dir ? flatten(entry.children) : [entry]);
}
