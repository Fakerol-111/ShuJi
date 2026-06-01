import { useState, useEffect, useRef } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { getRecentDirs, getRoundMetrics } from "../api";
import { useActiveDepts } from "../hooks/useActiveDepts";
import { useProject } from "../hooks/useProject";
import { useChat, type Tab } from "../hooks/useChat";
import ActivityBar from "../components/ActivityBar";
import Sidebar from "../components/Sidebar";
import DocPreview from "../components/DocPreview";
import DeptStatusBar from "../components/DeptStatusBar";
import LogBar from "../components/LogBar";
import ProjectOverview from "../components/ProjectOverview";
import SettingsMenu from "../components/SettingsMenu";
import HelpDrawer from "../components/HelpDrawer";
import ProjectPicker from "../components/ProjectPicker";
import ChatPanel from "../components/ChatPanel";
import { SealLogo } from "../components/SealLogo";
import { Tabs } from "../components/ui/Tabs";
import { Button } from "../components/ui/Button";
import DemoTour from "../components/DemoTour";
import WorkflowStatus from "../components/WorkflowTimeline";
import { Card } from "../components/ui/Card";
import { createDemoProject, getPendingApprovals } from "../api";
import type { ActivitySelection } from "../components/ActivityBar";

const STORAGE_KEY = "shuji_chat";
const CHAT_PANEL_MIN = 300;
const CHAT_PANEL_MAX = 600;

function loadSession() {
  try {
    const raw = sessionStorage.getItem(STORAGE_KEY);
    return raw ? JSON.parse(raw) : null;
  } catch { return null; }
}

export default function ProjectDashboard() {
  const session = loadSession();
  const activeDepts = Array.from(useActiveDepts());

  // Project state
  const { project, messages: initialMsgs, recentDirs, setRecentDirs, error: projError, setError: setProjError, loadProjectIntoState } = useProject();
  if (initialMsgs.length > 0 && session) session.msgs = initialMsgs;

  // Chat state
  const { messages, discussMsgs, discussing, tab, planInfo, error: chatError, setError: setChatError, setTab, handleSend, handleDiscuss, resetDiscuss, chatEndRef } = useChat(session?.msgs || []);

  // Save session on message changes
  try { sessionStorage.setItem(STORAGE_KEY, JSON.stringify({ msgs: messages, discuss: discussMsgs })); } catch {}

  // Demo flow state
  const [showDemoTour, setShowDemoTour] = useState(false);
  const [demoStartTime, setDemoStartTime] = useState<number | null>(null);
  const [demoSummary, setDemoSummary] = useState<{
    elapsed: string;
    tokens: number;
    cached: number;
    uncached: number;
  } | null>(null);
  const summaryShownRef = useRef(false);

  // ── Pending approvals: poll every 3s ────────────────────
  const [pendingApprovals, setPendingApprovals] = useState<string[]>([]);

  useEffect(() => {
    if (!project) { setPendingApprovals([]); return; }
    const fetch = () => {
      getPendingApprovals().then(setPendingApprovals).catch(() => {});
    };
    fetch();
    const timer = setInterval(fetch, 3000);
    return () => clearInterval(timer);
  }, [project?.working_dir, project]);

  // ── Demo flow: auto-send from WorkspaceSelect ──────────────
  useEffect(() => {
    if (!project) return;
    const isDemo = sessionStorage.getItem("shuji_demo");
    if (isDemo !== "true") return;
    sessionStorage.removeItem("shuji_demo");
    setDemoStartTime(Date.now());
    handleSend("修复 calc.py 中的 power 和 factorial 函数中的 bug，确保所有测试通过");
  }, [project, handleSend]);

  // ── Demo flow: show guided tour after auto-send ────────────
  useEffect(() => {
    if (!demoStartTime) return;
    const tourDone = localStorage.getItem("shuji_demo_tour_done");
    if (!tourDone) setShowDemoTour(true);
  }, [demoStartTime]);

  // ── Demo flow: detect completion and show summary ──────────
  useEffect(() => {
    if (!demoStartTime || summaryShownRef.current) return;
    if (!project?.working_dir?.includes("calc_demo")) return;

    // Workflow is idle when no active departments and no plan in progress
    const isIdle = activeDepts.length === 0 && !planInfo;
    if (!isIdle) return;

    // Wait at least 20s after start to avoid premature summary
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

  const error = projError || chatError;
  const clearError = () => { setProjError(""); setChatError(""); };

  // UI-only state
  const [activity, setActivity] = useState<ActivitySelection>("files");
  const [selectedDoc, setSelectedDoc] = useState<string | null>(null);
  const [logsExpanded, setLogsExpanded] = useState(false);
  const [chatWidth, setChatWidth] = useState(400);
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [showPicker, setShowPicker] = useState(false);
  const [pickerPath, setPickerPath] = useState("");
  const [pickerLoading, setPickerLoading] = useState(false);
  const [pickerError, setPickerError] = useState("");

  const openProjectPicker = () => {
    setPickerPath("");
    setPickerError("");
    getRecentDirs().then(setRecentDirs).catch((e) => setPickerError(String(e)));
    setShowPicker(true);
  };

  const handleBrowse = async () => {
    try {
      const selected = await open({ directory: true, multiple: false, title: "选择工作目录" });
      if (selected) setPickerPath(selected);
    } catch (e) { setPickerError(String(e)); }
  };

  const handleLoadProject = async (dir?: string) => {
    const path = dir || pickerPath.trim();
    if (!path) { setPickerError("请选择工作目录"); return; }
    setPickerLoading(true);
    setPickerError("");
    try {
      await loadProjectIntoState(path);
      setSelectedDoc(null);
      resetDiscuss();
      sessionStorage.removeItem(STORAGE_KEY);
      setShowPicker(false);
    } catch (e) { setPickerError(String(e)); }
    finally { setPickerLoading(false); }
  };

  const startResize = (e: React.MouseEvent) => {
    e.preventDefault();
    const startX = e.clientX;
    const startWidth = chatWidth;
    const move = (ev: MouseEvent) => setChatWidth(Math.max(CHAT_PANEL_MIN, Math.min(CHAT_PANEL_MAX, startWidth - (ev.clientX - startX))));
    const up = () => { document.removeEventListener("mousemove", move); document.removeEventListener("mouseup", up); };
    document.addEventListener("mousemove", move);
    document.addEventListener("mouseup", up);
  };

  const tabLabels: Record<Tab, string> = { decision: "决策", discuss: "廷议" };
  const tabSubtitles: Record<Tab, string> = { decision: "下达敕令，驱动各部门执行", discuss: "仅与内阁议政，不改代码、不写文档" };

  const handleConvertToCommand = (text: string) => {
    handleSend(text);
    setTab("decision");
  };

  const handleDemoProject = async () => {
    try {
      const project = await createDemoProject();
      await loadProjectIntoState(project.working_dir);
      setSelectedDoc(null);
      resetDiscuss();
      sessionStorage.removeItem(STORAGE_KEY);
      setTab("decision");
      // Auto-send a command so the user immediately sees departments working
      handleSend("修复 calc.py 中的 power 和 factorial 函数中的 bug，确保所有测试通过");
    } catch (e) {
      setChatError(String(e));
    }
  };

  return (
    <div className="h-screen bg-surface-paper flex flex-col overflow-hidden">
      <header className="bg-ink-900 border-b border-gold/30 shrink-0 h-12 px-4 flex items-center justify-between">
        <div className="flex items-center gap-3 min-w-0">
          <SealLogo size={20} />
          <h1 className="font-display text-base font-semibold text-ink-50 truncate">{project?.name || "枢机"}</h1>
          <span className="text-caption text-ink-500 font-mono truncate max-w-[520px]">{project?.working_dir}</span>
        </div>
        <div className="flex items-center gap-2">
          <Button variant="seal" className="text-xs !px-2 !py-1" onClick={handleDemoProject}>体验枢机</Button>
          <Button variant="ghost" className="text-xs !px-2 !py-1 text-ink-400" onClick={openProjectPicker}>打开项目</Button>
          <HelpDrawer />
          <SettingsMenu open={settingsOpen} setOpen={setSettingsOpen} />
        </div>
      </header>

      {error && <div className="px-4 py-2 bg-vermillion-light border-b border-vermillion/20 text-vermillion-dark text-ui shrink-0">{error}<button onClick={clearError} className="ml-2 font-bold">&times;</button></div>}

      <div className="flex-1 flex min-h-0 overflow-hidden">
        <ActivityBar selected={activity} onSelect={setActivity} onLogsClick={() => setLogsExpanded(true)} />
        {activity && project && <Sidebar mode={activity} projectDir={project.working_dir} selectedDoc={selectedDoc} onDocSelect={setSelectedDoc} />}
        <main className="flex-1 min-w-0 min-h-0 overflow-hidden flex flex-col">
          {project && (
            <WorkflowStatus
              phaseCount={project.phase_count}
              phases={project.phases}
              overall={typeof project.overall === "string" ? project.overall : String(project.overall)}
              activeDepts={activeDepts}
              planInfo={planInfo}
              pendingApprovals={pendingApprovals}
              onSelectDoc={setSelectedDoc}
            />
          )}
          <div className="flex-1 min-h-0 overflow-hidden">
            {demoSummary ? (
            <DemoSummaryCard summary={demoSummary} onOpenProject={openProjectPicker} />
          ) : project && selectedDoc ? (
            <DocPreview projectDir={project.working_dir} docPath={selectedDoc} />
          ) : (
            <ProjectOverview project={project} activeDepts={activeDepts} planInfo={planInfo} onOpenProject={openProjectPicker} />
          )}
        </div></main>
        <section className="relative bg-surface-paper border-l border-fold flex flex-col min-h-0 shrink-0" style={{ width: chatWidth }}>
          <div onMouseDown={startResize} className="absolute left-0 top-0 bottom-0 w-1 cursor-col-resize hover:bg-vermillion/40 transition-colors" />
          <div className="border-b border-fold bg-surface-elevated shrink-0 px-3 py-2">
            <Tabs
              tabs={[
                { key: "decision", label: tabLabels.decision },
                { key: "discuss", label: tabLabels.discuss },
              ]}
              activeKey={tab}
              onChange={(k) => setTab(k as Tab)}
            />
            <div className="text-ui text-ink-600 mt-1">{tabSubtitles[tab]}</div>
          </div>
          {!project ? <div className="flex-1 flex items-center justify-center text-body text-ink-400">请先开卷</div> : <ChatPanel tab={tab} messages={messages} discussMsgs={discussMsgs} discussing={discussing} planInfo={planInfo} onOption={(key, supplement) => handleSend(supplement ? `${key}\n${supplement}` : key)} onSend={handleSend} onDiscuss={handleDiscuss} onConvertToCommand={handleConvertToCommand} endRef={chatEndRef} />}
        </section>
      </div>

      <DeptStatusBar />
      <LogBar expanded={logsExpanded} onExpandedChange={setLogsExpanded} />
      {showPicker && <ProjectPicker recentDirs={recentDirs} pickerPath={pickerPath} pickerError={pickerError} pickerLoading={pickerLoading} setPickerPath={setPickerPath} onBrowse={handleBrowse} onLoad={handleLoadProject} onClose={() => setShowPicker(false)} />}
      {showDemoTour && <DemoTour onClose={() => setShowDemoTour(false)} />}
    </div>
  );
}

// ── Demo Summary Card ───────────────────────────────────
function DemoSummaryCard({ summary, onOpenProject }: {
  summary: { elapsed: string; tokens: number; cached: number; uncached: number };
  onOpenProject: () => void;
}) {
  const cacheRate = summary.tokens > 0
    ? Math.round((summary.cached / (summary.cached + summary.uncached)) * 100)
    : null;

  return (
    <div className="h-full overflow-y-auto surface-paper p-8">
      <Card variant="paper" className="max-w-3xl mx-auto p-6">
        <div className="text-center mb-2">
          <div className="w-12 h-12 mx-auto mb-3 rounded-full bg-jade-light flex items-center justify-center">
            <svg className="w-6 h-6 text-jade" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={2.5}>
              <path strokeLinecap="round" strokeLinejoin="round" d="M4.5 12.75l6 6 9-13.5" />
            </svg>
          </div>
          <h2 className="font-display text-display font-bold text-ink-900">Demo 完成</h2>
          <p className="text-body text-ink-600 mt-1">体验流程已结束，以下是本次 Demo 的概览。</p>
        </div>

        <section className="mb-6">
          <h3 className="font-display text-ui text-ink-600 font-semibold title-rule-gold mb-3">汇总</h3>
          <div className="grid grid-cols-1 sm:grid-cols-3 gap-3">
            <div className="bg-surface-parchment border border-fold rounded-lg p-3 text-center">
              <p className="text-caption text-ink-500 mb-1">耗时</p>
              <p className="font-display text-xl text-ink-900 font-bold">{summary.elapsed}</p>
            </div>
            <div className="bg-surface-parchment border border-fold rounded-lg p-3 text-center">
              <p className="text-caption text-ink-500 mb-1">Token 消耗</p>
              <p className="font-display text-xl text-ink-900 font-bold">{summary.tokens.toLocaleString()}</p>
              <p className="text-caption text-ink-500 mt-1">
                缓存 {summary.cached.toLocaleString()} / 未缓存 {summary.uncached.toLocaleString()}
              </p>
            </div>
            <div className="bg-surface-parchment border border-fold rounded-lg p-3 text-center">
              <p className="text-caption text-ink-500 mb-1">缓存命中率</p>
              <p className="font-display text-xl text-ink-900 font-bold">
                {cacheRate !== null ? `${cacheRate}%` : "N/A"}
              </p>
            </div>
          </div>
        </section>

        <section className="mb-6">
          <h3 className="font-display text-ui text-ink-600 font-semibold title-rule-gold mb-3">下一步</h3>
          <ul className="space-y-2 text-body text-ink-700">
            <li className="leading-relaxed">
              <strong>打开真实项目</strong> — 选择您的项目目录，枢机将根据需求自动规划并执行任务。
            </li>
            <li className="leading-relaxed">
              <strong>调整参与模式</strong> — 使用 <code className="text-vermillion bg-vermillion-light px-1 rounded text-ui">/level-2</code> 切换审批模式，让系统在关键节点等待您的确认。
            </li>
          </ul>
        </section>

        <div className="flex justify-center gap-3">
          <Button variant="secondary" onClick={onOpenProject}>
            打开真实项目
          </Button>
        </div>
      </Card>
    </div>
  );
}
