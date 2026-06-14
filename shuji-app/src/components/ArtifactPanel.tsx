import TabBar, { type TabInfo } from './TabBar';
import DocPreview from './DocPreview';
import ApprovalPromptCard from './ApprovalPromptCard';

interface ArtifactPanelProps {
  project: { working_dir: string };
  tabs: TabInfo[];
  activeIndex: number;
  activeDoc: TabInfo | null;
  hasTabs: boolean;
  pendingApprovals: string[];
  onSelectTab: (index: number) => void;
  onCloseTab: (index: number) => void;
  onClosePanel: () => void;
  onOpenApproval: (docPath: string) => void;
}

export default function ArtifactPanel({
  project,
  tabs,
  activeIndex,
  activeDoc,
  hasTabs,
  pendingApprovals,
  onSelectTab,
  onCloseTab,
  onClosePanel,
  onOpenApproval,
}: ArtifactPanelProps) {
  return (
    <div className="h-full flex flex-col min-h-0 min-w-0">
      <div className="shrink-0 flex items-center justify-between px-3 py-1.5 border-b border-fold bg-surface-parchment">
        <span className="text-caption font-medium text-ink-600 font-display">架阁</span>
        <button
          onClick={onClosePanel}
          className="shrink-0 w-5 h-5 flex items-center justify-center rounded text-caption text-ink-400 hover:text-ink-900 hover:bg-ink-200/60 transition-colors"
          title="关闭面板"
        >
          ✕
        </button>
      </div>
      {hasTabs && (
        <TabBar tabs={tabs} activeIndex={activeIndex} onSelect={onSelectTab} onClose={onCloseTab} />
      )}
      {hasTabs && activeDoc ? (
        <DocPreview
          key={activeDoc.path}
          projectDir={project.working_dir}
          docPath={activeDoc.path}
          initialTab={activeDoc.initialView}
          onClose={onClosePanel}
        />
      ) : pendingApprovals.length > 0 ? (
        <ApprovalPromptCard
          docPaths={pendingApprovals}
          projectDir={project.working_dir}
          onSelect={onOpenApproval}
        />
      ) : (
        <div className="flex-1 flex items-center justify-center p-6 text-center min-w-0">
          <div className="max-w-xs">
            <div className="text-ink-300 text-5xl font-serif mb-3">牍</div>
            <p className="text-caption text-ink-500">点击文档引用或阶段详情以预览</p>
          </div>
        </div>
      )}
    </div>
  );
}
