import DocTree from "./DocTree";
import TokenPanel from "./TokenPanel";
import ContextPanel from "./ContextPanel";
import CheckpointPanel from "./CheckpointPanel";
import AuditPanel from "./AuditPanel";
import type { ActivitySelection } from "./ActivityBar";

const headerLabel: Record<string, string> = {
  files: "架阁目录",
  stats: "度支",
  context: "文脉",
  archives: "存档",
  audit: "朝报",
};

function getHeader(mode: Exclude<ActivitySelection, null>): string {
  return headerLabel[mode] || mode;
}

function panel(mode: Exclude<ActivitySelection, null>, projectDir: string, selectedDoc: string | null, onDocSelect: (path: string) => void) {
  switch (mode) {
    case "files": return <DocTree projectDir={projectDir} selectedDoc={selectedDoc} onSelect={onDocSelect} />;
    case "stats": return <TokenPanel />;
    case "context": return <ContextPanel />;
    case "archives": return <CheckpointPanel />;
    case "audit": return <AuditPanel onDocSelect={onDocSelect} />;
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
    <aside className="w-60 bg-surface-parchment border-r border-fold shrink-0 flex flex-col min-h-0">
      <div className="h-9 px-3 border-b border-fold flex items-center font-display text-ui font-semibold text-ink-700">
        {getHeader(mode)}
      </div>
      <div className="flex-1 min-h-0 overflow-y-auto">
        {panel(mode, projectDir, selectedDoc, onDocSelect)}
      </div>
    </aside>
  );
}
