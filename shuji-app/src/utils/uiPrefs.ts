export type ExperienceLevel = 'beginner' | 'advanced';
export type UiMode = 'focus' | 'review' | 'inspect';

export type ActivitySelection =
  | 'files'
  | 'stats'
  | 'context'
  | 'archives'
  | 'audit'
  | 'graph'
  | null;

export interface UiPrefs {
  lastActivity?: ActivitySelection;
  lastUiMode?: UiMode;
  experienceLevel?: ExperienceLevel;
  projectOnboardingDone?: boolean;
  workflowCollapsed?: boolean;
}

const STORAGE_KEY = 'shuji_ui_prefs';

export function loadUiPrefs(): UiPrefs {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    return raw ? (JSON.parse(raw) as UiPrefs) : {};
  } catch {
    return {};
  }
}

export function saveUiPrefs(patch: Partial<UiPrefs>): UiPrefs {
  const next = { ...loadUiPrefs(), ...patch };
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(next));
  } catch {
    /* ignore */
  }
  return next;
}

export function getExperienceLevel(): ExperienceLevel {
  return loadUiPrefs().experienceLevel ?? 'beginner';
}

export function isProjectOnboardingDone(): boolean {
  return loadUiPrefs().projectOnboardingDone === true;
}

export function markProjectOnboardingDone(): void {
  saveUiPrefs({ projectOnboardingDone: true });
}

export function clearProjectOnboardingDone(): void {
  const prefs = loadUiPrefs();
  const { projectOnboardingDone: _, ...rest } = prefs;
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(rest));
  } catch {
    /* ignore */
  }
}
