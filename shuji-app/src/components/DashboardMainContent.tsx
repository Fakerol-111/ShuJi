import ActiveDeptStrip from './ActiveDeptStrip';
import WorkflowStatus from './WorkflowTimeline';
import WorkflowGraphView from './WorkflowGraph';
import DemoSummaryCard from './DemoSummaryCard';
import TabBar, { type TabInfo } from './TabBar';
import DocPreview from './DocPreview';
import ProjectOverview from './ProjectOverview';
import type { Project, PlanInfo } from '../types';
import type { ActivitySelection } from './ActivityBar';

interface Props {
  project: Project | null;
  activeDepts: string[];
  planInfo: PlanInfo | null;
  activity: ActivitySelection;
  pendingApprovals: string[];
  demoSummary: { elapsed: string; tokens: number; cached: number; uncached: number } | null;
  tabs: TabInfo[];
  activeIndex: number;
  activeDoc: TabInfo | null;
  hasTabs: boolean;
  setActiveIndex: (i: number) => void;
  closeTab: (i: number) => void;
  openTab: (path: string, initialView?: TabInfo['initialView']) => void;
  onOpenProject: () => void;
}

export default function DashboardMainContent({
  project,
  activeDepts,
  planInfo,
  activity,
  pendingApprovals,
  demoSummary,
  tabs,
  activeIndex,
  activeDoc,
  hasTabs,
  setActiveIndex,
  closeTab,
  openTab,
  onOpenProject,
}: Props) {
  return (
    <>
      <ActiveDeptStrip activeDepts={activeDepts} planInfo={planInfo} />
      {project && (
        <WorkflowStatus
          phaseCount={project.phase_count}
          phases={project.phases}
          overall={typeof project.overall === 'string' ? project.overall : String(project.overall)}
          activeDepts={activeDepts}
          planInfo={planInfo}
          pendingApprovals={pendingApprovals}
          onSelectDoc={(path) => openTab(path)}
        />
      )}
      <div className="flex-1 min-h-0 overflow-hidden flex flex-col">
        {activity === 'graph' ? (
          <WorkflowGraphView />
        ) : demoSummary ? (
          <DemoSummaryCard summary={demoSummary} onOpenProject={onOpenProject} />
        ) : (
          <>
            {hasTabs && (
              <TabBar
                tabs={tabs}
                activeIndex={activeIndex}
                onSelect={setActiveIndex}
                onClose={closeTab}
              />
            )}
            {hasTabs ? (
              <DocPreview
                key={activeDoc!.path}
                projectDir={project!.working_dir}
                docPath={activeDoc!.path}
                initialTab={activeDoc!.initialView}
                onClose={() => closeTab(activeIndex)}
              />
            ) : (
              <ProjectOverview
                project={project}
                activeDepts={activeDepts}
                planInfo={planInfo}
                onOpenProject={onOpenProject}
                onDocSelect={(path) => openTab(path)}
              />
            )}
          </>
        )}
      </div>
    </>
  );
}
