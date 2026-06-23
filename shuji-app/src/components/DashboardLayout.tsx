import { type ReactNode, useState, useCallback, useRef, useEffect } from 'react';
import { useTranslation } from 'react-i18next';
import { DeptEventsProvider } from '../hooks/useDeptEvents';
import { UsageStatsProvider } from '../hooks/useUsageStats';
import { SealLogo } from './SealLogo';
import ActivityBar from './ActivityBar';
import type { ActivitySelection } from './ActivityBar';
import Sidebar from './Sidebar';
import DutyBar from './DutyBar';
import type { Project } from '../types';

const ARTIFACT_MIN = 320;
const ARTIFACT_MAX = 480;
const ARTIFACT_DEFAULT = 380;
const ARTIFACT_PREF_KEY = 'shuji_artifact_width';

function loadArtifactWidth(): number {
  try {
    const raw = localStorage.getItem(ARTIFACT_PREF_KEY);
    if (raw) {
      const v = parseInt(raw, 10);
      if (!isNaN(v) && v >= ARTIFACT_MIN && v <= ARTIFACT_MAX) return v;
    }
  } catch {}
  return ARTIFACT_DEFAULT;
}

function saveArtifactWidth(v: number) {
  try {
    localStorage.setItem(ARTIFACT_PREF_KEY, String(v));
  } catch {}
}

interface Props {
  project: Project | null;
  error: string;
  clearError: () => void;
  activity: ActivitySelection;
  onActivity: (a: ActivitySelection) => void;
  activeDocPath: string | null;
  onDocSelect: (path: string) => void;
  onShowDiff: (path: string) => void;
  pendingApprovalsCount: number;
  headerLeft?: ReactNode;
  headerRight?: ReactNode;
  agentStream: ReactNode;
  artifactPanel?: ReactNode;
  artifactOpen: boolean;
  picker?: ReactNode;
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
  pendingApprovalsCount,
  headerLeft,
  headerRight,
  agentStream,
  artifactPanel,
  artifactOpen,
  picker,
  demoTour,
}: Props) {
  const { t } = useTranslation();
  const [artifactWidth, setArtifactWidth] = useState(loadArtifactWidth);
  const dragRef = useRef(false);
  const startXRef = useRef(0);
  const startWidthRef = useRef(0);

  const onDragStart = useCallback(
    (e: React.MouseEvent) => {
      dragRef.current = true;
      startXRef.current = e.clientX;
      startWidthRef.current = artifactWidth;
      document.body.style.cursor = 'col-resize';
      document.body.style.userSelect = 'none';
    },
    [artifactWidth]
  );

  useEffect(() => {
    const onMove = (e: MouseEvent) => {
      if (!dragRef.current) return;
      const delta = e.clientX - startXRef.current;
      const newWidth = Math.max(
        ARTIFACT_MIN,
        Math.min(ARTIFACT_MAX, startWidthRef.current - delta)
      );
      setArtifactWidth(newWidth);
    };
    const onUp = () => {
      if (!dragRef.current) return;
      dragRef.current = false;
      document.body.style.cursor = '';
      document.body.style.userSelect = '';
      saveArtifactWidth(artifactWidth);
    };
    window.addEventListener('mousemove', onMove);
    window.addEventListener('mouseup', onUp);
    return () => {
      window.removeEventListener('mousemove', onMove);
      window.removeEventListener('mouseup', onUp);
    };
  }, [artifactWidth]);

  return (
    <DeptEventsProvider>
      <UsageStatsProvider>
        <div className="h-screen bg-surface-paper flex flex-col overflow-hidden">
          <header className="bg-ink-900 border-b border-gold/30 shrink-0 h-12 px-4 flex items-center justify-between">
            <div className="flex items-center gap-3 min-w-0">
              <SealLogo size={20} />
              {headerLeft ?? (
                <>
                  <h1 className="font-display text-base font-semibold text-ink-50 truncate">
                    {project?.name || t('app.name')}
                  </h1>
                  <span className="text-caption text-ink-500 font-mono truncate max-w-[520px]">
                    {project?.working_dir}
                  </span>
                </>
              )}
            </div>
            <div className="flex items-center gap-2">{headerRight}</div>
          </header>

          {error && (
            <div className="px-4 py-2 bg-vermillion-light border-b border-vermillion/20 text-vermillion-dark text-ui shrink-0">
              {error}
              <button onClick={clearError} className="ml-2 font-bold">
                &times;
              </button>
            </div>
          )}

          <div className="flex-1 flex min-h-0 overflow-hidden">
            <ActivityBar
              selected={activity}
              onSelect={onActivity}
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
            <div className="flex-1 flex min-h-0">
              {activity === 'graph' ? (
                <div className="flex-1 min-h-0">{artifactPanel}</div>
              ) : (
                <div className="flex-1 min-w-0 min-h-0 flex flex-col">{agentStream}</div>
              )}
              {activity !== 'graph' && artifactOpen && artifactPanel && (
                <>
                  <div
                    className="w-1 shrink-0 cursor-col-resize bg-fold hover:bg-gold/40 active:bg-gold/60 transition-colors"
                    onMouseDown={onDragStart}
                  />
                  <aside
                    className="shrink-0 flex flex-col min-h-0 min-w-0 bg-surface-paper overflow-hidden"
                    style={{ width: artifactWidth }}
                  >
                    {artifactPanel}
                  </aside>
                </>
              )}
            </div>
          </div>

          <DutyBar />
          {picker}
          {demoTour}
        </div>
      </UsageStatsProvider>
    </DeptEventsProvider>
  );
}
