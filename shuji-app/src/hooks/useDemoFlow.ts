/**
 * Demo flow state machine for ProjectDashboard.
 * Manages demo project creation, auto-send, tour, and completion summary.
 */
import { useState, useEffect, useRef, useCallback } from 'react';
import { useTranslation } from 'react-i18next';
import { createDemoProject, getRoundMetrics, runMockWorkflow } from '../api';
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
  setTab?: (tab: 'decision' | 'discuss') => void,
  setMessages?: (msgs: any[] | ((prev: any[]) => any[])) => void
) {
  const { t, i18n } = useTranslation();
  const lang = i18n.language?.startsWith('en') ? 'en' : 'zh';
  const [showDemoTour, setShowDemoTour] = useState(false);
  const [demoCreating, setDemoCreating] = useState(false);
  const [demoStartTime, setDemoStartTime] = useState<number | null>(null);
  const [demoSummary, setDemoSummary] = useState<DemoSummary | null>(null);
  const [mockScenario, setMockScenario] = useState<string | null>(null);
  const summaryShownRef = useRef(false);
  const demoMsg = t('demo.demoMessage');

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
      setDemoStartTime(Date.now());
      // Offline mock mode: load pre-recorded scenario instead of calling real API
      const userMsg = demoMsg;
      if (project.working_dir && setMessages) {
        runMockWorkflow(project.working_dir, scenario)
          .then((msgs) => {
            const ts = new Date().toISOString();
            const userMessage = {
              id: crypto.randomUUID(),
              role: '皇帝',
              content: userMsg,
              options: [],
              documents: [],
              timestamp: ts,
            };
            const normalizedMsgs = msgs.map((m) => ({
              ...m,
              documents: (m as any).documents || [],
            }));
            setMessages([userMessage, ...normalizedMsgs]);
          })
          .catch((err) => {
            console.error('离线演示加载失败:', err);
            // fallback: send as real message
            handleSend(userMsg);
          });
      } else {
        handleSend(userMsg);
      }
      return;
    }
    setDemoStartTime(Date.now());
    handleSend(demoMsg);
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
    const elapsedStr = lang === 'en'
      ? `${minutes}m ${seconds}s`
      : `${minutes}分${seconds}秒`;

    getRoundMetrics()
      .then((metrics) => {
        setDemoSummary({
          elapsed: elapsedStr,
          tokens: metrics?.total_tokens || 0,
          cached: metrics?.cached_prompt_tokens || 0,
          uncached: metrics?.uncached_prompt_tokens || 0,
        });
      })
      .catch(() => {
        setDemoSummary({
          elapsed: elapsedStr,
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
      handleSend(demoMsg);
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
