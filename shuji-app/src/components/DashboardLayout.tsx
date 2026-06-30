import { type ReactNode, useState, useCallback, useRef, useEffect } from 'react';
import { useTranslation } from 'react-i18next';
import { DeptEventsProvider } from '../hooks/useDeptEvents';
import { UsageStatsProvider } from '../hooks/useUsageStats';
import { SealLogo } from './SealLogo';
import type { ActivitySelection } from '../utils/uiPrefs';
import ActivityBar from './ActivityBar';
import Sidebar from './Sidebar';
import DutyBar from './DutyBar';
import type { Project } from '../types';

const ARTIFACT_MIN = 320;
const ARTIFACT_MAX = 680;
const ARTIFACT_DEFAULT = 520;
const RESERVED_MIN_MAIN = 420;
const ARTIFACT_PREF_KEY = 'shuji_artifact_width';

function clampArtifactWidth(width: number, available: number): number {
  const maxByViewport = Math.max(0, available - RESERVED_MIN_MAIN);
  const max = Math.min(ARTIFACT_MAX, maxByViewport);
  if (max < ARTIFACT_MIN) return 0;
  return Math.max(ARTIFACT_MIN, Math.min(max, width));
}

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
  approvalBanner?: ReactNode;
  headerLeft?: ReactNode;
  headerRight?: ReactNode;
  agentStream: ReactNode;
  artifactPanel?: ReactNode;
  artifactOpen: boolean;
  picker?: ReactNode;
  demoTour?: ReactNode;
  beginnerMode?: boolean;
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
  approvalBanner,
  headerLeft,
  headerRight,
  agentStream,
  artifactPanel,
  artifactOpen,
  picker,
  demoTour,
  beginnerMode = false,
}: Props) {
  const { t } = useTranslation();
  const [artifactWidth, setArtifactWidth] = useState(loadArtifactWidth);
  const [isResizing, setIsResizing] = useState(false);

  const mainContainerRef = useRef<HTMLDivElement>(null);
  const latestWidthRef = useRef(artifactWidth);
  const rafIdRef = useRef(0);

  // Sync latestWidthRef with state
  latestWidthRef.current = artifactWidth;

  // ResizeObserver: clamp artifact width when available space changes
  useEffect(() => {
    const el = mainContainerRef.current;
    if (!el) return;
    const ro = new ResizeObserver((entries) => {
      const entry = entries[0];
      if (!entry) return;
      const available = entry.contentRect.width;
      const clamped = clampArtifactWidth(latestWidthRef.current, available);
      if (clamped !== latestWidthRef.current) {
        latestWidthRef.current = clamped;
        setArtifactWidth(clamped);
        saveArtifactWidth(clamped);
      }
    });
    ro.observe(el);
    return () => ro.disconnect();
  }, []);

  // Pointer-based drag with RAF throttling
  const onDragStart = useCallback((e: React.PointerEvent) => {
    e.currentTarget.setPointerCapture(e.pointerId);
    setIsResizing(true);
    document.body.style.cursor = 'col-resize';
    document.body.style.userSelect = 'none';
  }, []);

  const onDragMove = useCallback((e: React.PointerEvent) => {
    if (!e.currentTarget.hasPointerCapture(e.pointerId)) return;
    if (rafIdRef.current) cancelAnimationFrame(rafIdRef.current);
    rafIdRef.current = requestAnimationFrame(() => {
      const delta = e.movementX;
      const newWidth = Math.max(
        ARTIFACT_MIN,
        Math.min(ARTIFACT_MAX, latestWidthRef.current - delta)
      );
      latestWidthRef.current = newWidth;
      setArtifactWidth(newWidth);
    });
  }, []);

  const onDragEnd = useCallback((e: React.PointerEvent) => {
    if (!e.currentTarget.hasPointerCapture(e.pointerId)) return;
    e.currentTarget.releasePointerCapture(e.pointerId);
    if (rafIdRef.current) cancelAnimationFrame(rafIdRef.current);
    setIsResizing(false);
    document.body.style.cursor = '';
    document.body.style.userSelect = '';
    saveArtifactWidth(latestWidthRef.current);
  }, []);

  return (
    <DeptEventsProvider>
      <UsageStatsProvider>
        <div
          className={`h-screen bg-surface-paper flex flex-col overflow-hidden${isResizing ? ' is-resizing' : ''}`}
        >
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

          {approvalBanner}

          <div className="flex-1 flex min-h-0 min-w-0 overflow-hidden">
            <ActivityBar
              selected={activity}
              onSelect={onActivity}
              pendingApprovalsCount={pendingApprovalsCount}
              beginnerMode={beginnerMode}
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
            <div ref={mainContainerRef} className="flex-1 flex min-w-0 min-h-0 overflow-hidden">
              {activity === 'graph' ? (
                <div className="flex-1 min-w-0 min-h-0 overflow-hidden">{artifactPanel}</div>
              ) : (
                <div className="flex-1 min-w-0 min-h-0 flex flex-col">{agentStream}</div>
              )}
              {activity !== 'graph' && artifactOpen && artifactPanel && (
                <>
                  <div
                    className="w-1 shrink-0 cursor-col-resize bg-fold hover:bg-gold/40 active:bg-gold/60 transition-colors"
                    onPointerDown={onDragStart}
                    onPointerMove={onDragMove}
                    onPointerUp={onDragEnd}
                  />
                  <aside
                    className="shrink-0 flex flex-col min-h-0 min-w-0 bg-surface-paper overflow-hidden"
                    style={{ width: artifactWidth, maxWidth: ARTIFACT_MAX }}
                  >
                    {artifactPanel}
                  </aside>
                </>
              )}
            </div>
          </div>

          <DutyBar projectDir={project?.working_dir} />
          {picker}
          {demoTour}
        </div>
      </UsageStatsProvider>
    </DeptEventsProvider>
  );
}
