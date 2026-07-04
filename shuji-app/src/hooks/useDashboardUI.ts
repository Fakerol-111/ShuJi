/**
 * Dashboard UI state hook — centralizes all UI-level state for ProjectDashboard.
 *
 * Extracts the following concerns from ProjectDashboard:
 * - Activity panel selection (files, stats, context, etc.)
 * - UI mode (focus / review / inspect)
 * - Experience level (beginner / advanced)
 * - Artifact panel open/close
 * - Settings page open/close
 * - Project onboarding modal
 * - Keyboard shortcuts (Ctrl+B, Ctrl+\, Escape)
 * - UI prefs persistence
 */
import { useState, useEffect, useCallback } from 'react';
import {
  loadUiPrefs,
  saveUiPrefs,
  getExperienceLevel,
  type ExperienceLevel,
  type ActivitySelection,
} from '../utils/uiPrefs';

export interface DashboardUIState {
  // Activity panel
  activity: ActivitySelection;
  setActivity: React.Dispatch<React.SetStateAction<ActivitySelection>>;
  onActivity: (a: ActivitySelection) => void;

  // UI mode
  uiMode: 'focus' | 'review' | 'inspect';
  setUiMode: React.Dispatch<React.SetStateAction<'focus' | 'review' | 'inspect'>>;

  // Experience level
  experienceLevel: ExperienceLevel;
  setExperienceLevel: React.Dispatch<React.SetStateAction<ExperienceLevel>>;
  onExperienceLevelChange: (level: ExperienceLevel) => void;
  beginnerMode: boolean;

  // Panel visibility
  artifactOpen: boolean;
  setArtifactOpen: React.Dispatch<React.SetStateAction<boolean>>;
  settingsOpen: boolean;
  setSettingsOpen: React.Dispatch<React.SetStateAction<boolean>>;
  showProjectOnboarding: boolean;
  setShowProjectOnboarding: React.Dispatch<React.SetStateAction<boolean>>;

  // Convenience
  openArtifact: (path?: string) => void;
}

export function useDashboardUI(
  hasTabs: boolean,
  pendingApprovals: string[],
  openTab: (path: string, initialView?: 'content' | 'diff' | 'lineage') => void,
  docIdToPath: (docId: string) => string
): DashboardUIState {
  const initialUiPrefs = loadUiPrefs();

  const [activity, setActivity] = useState<ActivitySelection>(initialUiPrefs.lastActivity ?? null);
  const [uiMode, setUiMode] = useState<'focus' | 'review' | 'inspect'>(
    initialUiPrefs.lastUiMode || 'focus'
  );
  const [experienceLevel, setExperienceLevel] = useState<ExperienceLevel>(
    () => initialUiPrefs.experienceLevel ?? getExperienceLevel()
  );
  const [artifactOpen, setArtifactOpen] = useState(false);
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [showProjectOnboarding, setShowProjectOnboarding] = useState(false);

  const beginnerMode = experienceLevel === 'beginner';

  // Keyboard shortcuts
  useEffect(() => {
    const onKeyDown = (e: KeyboardEvent) => {
      if (e.key === 'b' && (e.ctrlKey || e.metaKey)) {
        e.preventDefault();
        setActivity((prev) => (prev === 'files' ? null : 'files'));
      }
      if (e.key === '\\' && (e.ctrlKey || e.metaKey)) {
        e.preventDefault();
        setArtifactOpen((prev) => !prev);
      }
      if (e.key === 'Escape' && artifactOpen && uiMode !== 'review') {
        setArtifactOpen(false);
      }
    };
    window.addEventListener('keydown', onKeyDown);
    return () => window.removeEventListener('keydown', onKeyDown);
  }, [artifactOpen, uiMode]);

  // Auto-open artifact panel when tabs exist
  useEffect(() => {
    if (hasTabs) setArtifactOpen(true);
  }, [hasTabs]);

  // Switch to review mode when pending approvals arrive
  useEffect(() => {
    if (pendingApprovals.length === 0) return;
    setUiMode('review');
    setArtifactOpen(true);
    openTab(docIdToPath(pendingApprovals[0]));
  }, [pendingApprovals, openTab]);

  const handleActivity = useCallback(
    (a: ActivitySelection) => {
      if (a === null || a === activity) {
        setActivity(null);
        setUiMode('focus');
      } else {
        setActivity(a);
        setUiMode('inspect');
      }
      saveUiPrefs({
        lastUiMode: a === null || a === activity ? 'focus' : 'inspect',
        lastActivity: a === null || a === activity ? null : a,
      });
    },
    [activity]
  );

  const handleExperienceLevelChange = useCallback(
    (level: ExperienceLevel) => {
      setExperienceLevel(level);
      saveUiPrefs({ experienceLevel: level });
      if (
        level === 'beginner' &&
        activity &&
        (activity === 'stats' ||
          activity === 'context' ||
          activity === 'archives' ||
          activity === 'audit' ||
          activity === 'graph')
      ) {
        setActivity(null);
        setUiMode('focus');
      }
    },
    [activity]
  );

  const openArtifact = useCallback(
    (path?: string) => {
      if (path) openTab(path);
      setArtifactOpen(true);
    },
    [openTab]
  );

  return {
    activity,
    setActivity,
    onActivity: handleActivity,
    uiMode,
    setUiMode,
    experienceLevel,
    setExperienceLevel,
    onExperienceLevelChange: handleExperienceLevelChange,
    beginnerMode,
    artifactOpen,
    setArtifactOpen,
    settingsOpen,
    setSettingsOpen,
    showProjectOnboarding,
    setShowProjectOnboarding,
    openArtifact,
  };
}
