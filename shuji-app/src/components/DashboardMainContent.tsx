import { useEffect, useState } from 'react';
import ActiveDeptStrip from './ActiveDeptStrip';
import WorkflowStatus from './WorkflowTimeline';
import WorkflowGraphView from './WorkflowGraph';
import DemoSummaryCard from './DemoSummaryCard';
import TabBar, { type TabInfo } from './TabBar';
import DocPreview from './DocPreview';
import ProjectOverview from './ProjectOverview';
import { getRoundMetrics } from '../api';
import type { Project, PlanInfo, RoundMetrics } from '../types';
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
  const [roundMetrics, setRoundMetrics] = useState<RoundMetrics | null>(null);
  const [elapsed, setElapsed] = useState('');

  useEffect(() => {
    const load = () => {
      getRoundMetrics()
        .then((m) => setRoundMetrics(m))
        .catch(() => {});
    };
    load();
    const timer = window.setInterval(load, 3000);
    return () => window.clearInterval(timer);
  }, []);

  useEffect(() => {
    if (!roundMetrics || roundMetrics.started_at <= 0) {
      setElapsed('');
      return;
    }
    const tick = () => {
      const secs = Math.floor((Date.now() - roundMetrics.started_at) / 1000);
      if (secs < 60) setElapsed(`${secs}s`);
      else if (secs < 3600) setElapsed(`${Math.floor(secs / 60)}m${secs % 60}s`);
      else setElapsed(`${Math.floor(secs / 3600)}h${Math.floor((secs % 3600) / 60)}m`);
    };
    tick();
    const timer = window.setInterval(tick, 1000);
    return () => clearInterval(timer);
  }, [roundMetrics]);

  const totalStageCount = project ? project.phase_count : 0;
  const completedStageCount = project
    ? project.phases.filter((p) => p.execution === 'Completed' || p.execution === 'MinorIssue')
        .length
    : 0;

  return (
    <>
      <ActiveDeptStrip
        activeDepts={activeDepts}
        planInfo={planInfo}
        roundMetrics={roundMetrics}
        elapsed={elapsed}
        totalStageCount={totalStageCount}
        completedStageCount={completedStageCount}
      />
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
