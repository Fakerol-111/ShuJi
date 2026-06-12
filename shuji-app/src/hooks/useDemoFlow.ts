/**
 * Demo flow state machine for ProjectDashboard.
 * Manages demo project creation, auto-send, tour, and completion summary.
 */
import { useState, useEffect, useRef, useCallback } from 'react';
import { createDemoProject, getRoundMetrics } from '../api';
import { formatError } from '../utils/error';

export interface DemoSummary {
  elapsed: string;
  tokens: number;
  cached: number;
  uncached: number;
}

export function useDemoFlow(
  project: { working_dir?: string } | null,
  activeDepts: string[],
  planInfo: { batches: { status: string }[] } | null,
  handleSend: (text: string) => void,
  loadProjectIntoState: (path: string) => Promise<void>,
  resetDiscuss: () => void,
  setTab?: (tab: 'decision' | 'discuss') => void
) {
  const [showDemoTour, setShowDemoTour] = useState(false);
  const [demoCreating, setDemoCreating] = useState(false);
  const [demoStartTime, setDemoStartTime] = useState<number | null>(null);
  const [demoSummary, setDemoSummary] = useState<DemoSummary | null>(null);
  const [mockScenario, setMockScenario] = useState<string | null>(null);
  const summaryShownRef = useRef(false);

  // ── Demo flow: auto-send from WorkspaceSelect ──────────────
  useEffect(() => {
    if (!project) return;
    const isDemo = sessionStorage.getItem('shuji_demo');
    if (isDemo !== 'true') return;
    sessionStorage.removeItem('shuji_demo');
    const scenario = sessionStorage.getItem('shuji_mock_scenario');
    sessionStorage.removeItem('shuji_mock_scenario');
    if (scenario) {
      setMockScenario(scenario);
    }
    setDemoStartTime(Date.now());
    handleSend('修复 calc.py 中的 power 和 factorial 函数中的 bug，确保所有测试通过');
  }, [project, handleSend]);

  // ── Demo flow: show guided tour after auto-send ────────────
  useEffect(() => {
    if (!demoStartTime) return;
    const tourDone = localStorage.getItem('shuji_demo_tour_done');
    if (!tourDone) setShowDemoTour(true);
  }, [demoStartTime]);

  // ── Demo flow: detect completion and show summary ──────────
  useEffect(() => {
    if (!demoStartTime || summaryShownRef.current) return;
    if (!project?.working_dir?.includes('calc_demo')) return;

    const isIdle = activeDepts.length === 0 && !planInfo;
    if (!isIdle) return;
    if (Date.now() - demoStartTime < 20000) return;

    summaryShownRef.current = true;
    const elapsed = Math.round((Date.now() - demoStartTime) / 1000);
    const minutes = Math.floor(elapsed / 60);
    const seconds = elapsed % 60;

    getRoundMetrics()
      .then((metrics) => {
        setDemoSummary({
          elapsed: `${minutes}分${seconds}秒`,
          tokens: metrics?.total_tokens || 0,
          cached: metrics?.cached_prompt_tokens || 0,
          uncached: metrics?.uncached_prompt_tokens || 0,
        });
      })
      .catch(() => {
        setDemoSummary({
          elapsed: `${minutes}分${seconds}秒`,
          tokens: 0,
          cached: 0,
          uncached: 0,
        });
      });
  }, [activeDepts.length, planInfo, demoStartTime, project?.working_dir]);

  const handleDemoProject = useCallback(async () => {
    setDemoCreating(true);
    try {
      const proj = await createDemoProject();
      await loadProjectIntoState(proj.working_dir);
      setDemoStartTime(Date.now());
      resetDiscuss();
      sessionStorage.removeItem('shuji_chat');
      if (setTab) setTab('decision');
      handleSend('修复 calc.py 中的 power 和 factorial 函数中的 bug，确保所有测试通过');
    } catch (e) {
      // Error is surfaced through loadProjectIntoState or caller
      console.error(formatError(e));
    } finally {
      setDemoCreating(false);
    }
  }, [handleSend, loadProjectIntoState, resetDiscuss, setTab]);

  return {
    showDemoTour,
    setShowDemoTour,
    demoCreating,
    demoSummary,
    mockScenario,
    handleDemoProject,
  };
}
