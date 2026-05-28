import DocTree from "./DocTree";
import TokenPanel from "./TokenPanel";
import type { ActivitySelection } from "./ActivityBar";

interface SidebarProps {
  mode: Exclude<ActivitySelection, null>;
  projectDir: string;
  selectedDoc: string | null;
  onDocSelect: (path: string) => void;
}

export default function Sidebar({ mode, projectDir, selectedDoc, onDocSelect }: SidebarProps) {
  return (
    <aside className="w-60 bg-white border-r border-ink-200 shrink-0 flex flex-col min-h-0">
      <div className="h-9 px-3 border-b border-ink-200 flex items-center text-xs font-semibold text-ink-700 bg-ink-50">
        {mode === "files" ? "项目文件" : "Token 统计"}
      </div>
      <div className="flex-1 min-h-0 overflow-y-auto">
        {mode === "files" ? (
          <DocTree projectDir={projectDir} selectedDoc={selectedDoc} onSelect={onDocSelect} />
        ) : (
          <TokenPanel />
        )}
      </div>
    </aside>
  );
}
