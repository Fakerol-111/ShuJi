import DocTree from "./DocTree";
import TokenPanel from "./TokenPanel";
import ContextPanel from "./ContextPanel";
import type { ActivitySelection } from "./ActivityBar";

function headerLabel(mode: Exclude<ActivitySelection, null>): string {
  switch (mode) {
    case "files": return "项目文件";
    case "stats": return "Token 统计";
    case "context": return "文脉";
  }
}

function panel(mode: Exclude<ActivitySelection, null>, projectDir: string, selectedDoc: string | null, onDocSelect: (path: string) => void) {
  switch (mode) {
    case "files": return <DocTree projectDir={projectDir} selectedDoc={selectedDoc} onSelect={onDocSelect} />;
    case "stats": return <TokenPanel />;
    case "context": return <ContextPanel />;
  }
}

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
        {headerLabel(mode)}
      </div>
      <div className="flex-1 min-h-0 overflow-y-auto">
        {panel(mode, projectDir, selectedDoc, onDocSelect)}
      </div>
    </aside>
  );
}
