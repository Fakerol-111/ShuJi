import { type ReactNode } from 'react';
import { DeptActiveProvider } from '../hooks/useActiveDepts';
import { DeptEventsProvider } from '../hooks/useDeptEvents';
import { SealLogo } from './SealLogo';
import ActivityBar from './ActivityBar';
import type { ActivitySelection } from './ActivityBar';
import Sidebar from './Sidebar';
import DeptStatusBar from './DeptStatusBar';
import LogBar from './LogBar';
import type { Project } from '../types';

interface Props {
  project: Project | null;
  error: string;
  clearError: () => void;
  activity: ActivitySelection;
  onActivity: (a: ActivitySelection) => void;
  activeDocPath: string | null;
  onDocSelect: (path: string) => void;
  onShowDiff: (path: string) => void;
  logsExpanded: boolean;
  onLogsExpanded: (v: boolean) => void;
  pendingApprovalsCount: number;
  /** 左上角按钮 */
  headerLeft?: ReactNode;
  /** 右上角按钮区 */
  headerRight?: ReactNode;
  /** 主内容区（ActivityBar/Sidebar 右侧） */
  mainContent: ReactNode;
  /** 聊天面板 */
  chatPanel: ReactNode;
  /** 项目选择器 modal */
  picker?: ReactNode;
  /** Demo 导览 */
  demoTour?: ReactNode;
}

export default function DashboardLayout({
  project,
  error,
  clearError,
  activity,
  onActivity,
  activeDocPath,
  onDocSelect,
  onShowDiff,
  logsExpanded,
  onLogsExpanded,
  pendingApprovalsCount,
  headerLeft,
  headerRight,
  mainContent,
  chatPanel,
  picker,
  demoTour,
}: Props) {
  return (
    <DeptActiveProvider>
      <DeptEventsProvider>
        <div className="h-screen bg-surface-paper flex flex-col overflow-hidden">
          {/* ── Header ────────────────────────────────────────── */}
          <header className="bg-ink-900 border-b border-gold/30 shrink-0 h-12 px-4 flex items-center justify-between">
            <div className="flex items-center gap-3 min-w-0">
              <SealLogo size={20} />
              {headerLeft ?? (
                <>
                  <h1 className="font-display text-base font-semibold text-ink-50 truncate">
                    {project?.name || '枢机'}
                  </h1>
                  <span className="text-caption text-ink-500 font-mono truncate max-w-[520px]">
                    {project?.working_dir}
                  </span>
                </>
              )}
            </div>
            <div className="flex items-center gap-2">{headerRight}</div>
          </header>

          {/* ── Error banner ──────────────────────────────────── */}
          {error && (
            <div className="px-4 py-2 bg-vermillion-light border-b border-vermillion/20 text-vermillion-dark text-ui shrink-0">
              {error}
              <button onClick={clearError} className="ml-2 font-bold">
                &times;
              </button>
            </div>
          )}

          {/* ── Main area ─────────────────────────────────────── */}
          <div className="flex-1 flex min-h-0 overflow-hidden">
            <ActivityBar
              selected={activity}
              onSelect={onActivity}
              onLogsClick={() => onLogsExpanded(true)}
              pendingApprovalsCount={pendingApprovalsCount}
            />
            {activity && activity !== 'graph' && project && (
              <Sidebar
                mode={activity}
                projectDir={project.working_dir}
                selectedDoc={activeDocPath}
                onDocSelect={onDocSelect}
                onShowDiff={onShowDiff}
              />
            )}
            <main className="flex-1 min-w-0 min-h-0 overflow-hidden flex flex-col">
              {mainContent}
            </main>
            {chatPanel}
          </div>

          {/* ── Footer bars ──────────────────────────────────── */}
          <DeptStatusBar />
          <LogBar expanded={logsExpanded} onExpandedChange={onLogsExpanded} />
          {picker}
          {demoTour}
        </div>
      </DeptEventsProvider>
    </DeptActiveProvider>
  );
}
