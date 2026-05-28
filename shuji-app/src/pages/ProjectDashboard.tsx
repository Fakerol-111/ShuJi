import { useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { getRecentDirs } from "../api";
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
import ProjectPicker from "../components/ProjectPicker";
import ChatPanel from "../components/ChatPanel";
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

  const error = projError || chatError;
  const clearError = () => { setProjError(""); setChatError(""); };

  // UI-only state
  const [activity, setActivity] = useState<ActivitySelection>("files");
  const [selectedDoc, setSelectedDoc] = useState<string | null>(null);
  const [logsExpanded, setLogsExpanded] = useState(false);
  const [chatWidth, setChatWidth] = useState(400);
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [cfgKey, setCfgKey] = useState("");
  const [cfgUrl, setCfgUrl] = useState("");
  const [cfgModel, setCfgModel] = useState("");
  const [cfgMsg, setCfgMsg] = useState("");
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

  const tabLabels: Record<Tab, string> = { decision: "决策", discuss: "讨论" };

  return (
    <div className="h-screen bg-ink-50 flex flex-col overflow-hidden">
      <header className="bg-ink-900 border-b border-ink-800 shrink-0 h-11 px-4 flex items-center justify-between">
        <div className="flex items-center gap-3 min-w-0">
          <h1 className="text-sm font-bold text-ink-50 tracking-wide truncate">{project?.name || "枢机"}</h1>
          <span className="text-[11px] text-ink-500 font-mono truncate max-w-[520px]">{project?.working_dir}</span>
        </div>
        <div className="flex items-center gap-2">
          <button onClick={openProjectPicker} className="text-xs px-2 py-1 text-ink-400 hover:text-ink-100 hover:bg-ink-800 rounded">打开项目</button>
          <SettingsMenu open={settingsOpen} setOpen={setSettingsOpen} cfgKey={cfgKey} cfgUrl={cfgUrl} cfgModel={cfgModel} cfgMsg={cfgMsg} setCfgKey={setCfgKey} setCfgUrl={setCfgUrl} setCfgModel={setCfgModel} setCfgMsg={setCfgMsg} />
        </div>
      </header>

      {error && <div className="px-4 py-2 bg-vermillion-light border-b border-vermillion/20 text-vermillion-dark text-sm shrink-0">{error}<button onClick={clearError} className="ml-2 font-bold">&times;</button></div>}

      <div className="flex-1 flex min-h-0 overflow-hidden">
        <ActivityBar selected={activity} onSelect={setActivity} onLogsClick={() => setLogsExpanded(true)} />
        {activity && project && <Sidebar mode={activity} projectDir={project.working_dir} selectedDoc={selectedDoc} onDocSelect={setSelectedDoc} />}
        <main className="flex-1 min-w-0 min-h-0 overflow-hidden">
          {project && selectedDoc ? <DocPreview projectDir={project.working_dir} docPath={selectedDoc} /> : <ProjectOverview project={project} activeDepts={activeDepts} planInfo={planInfo} onOpenProject={openProjectPicker} />}
        </main>
        <section className="relative bg-ink-50 border-l border-ink-200 flex flex-col min-h-0 shrink-0" style={{ width: chatWidth }}>
          <div onMouseDown={startResize} className="absolute left-0 top-0 bottom-0 w-1 cursor-col-resize hover:bg-vermillion/40 transition-colors" />
          <div className="h-9 border-b border-ink-200 bg-white flex items-center px-3 gap-1 shrink-0">
            {(["decision", "discuss"] as Tab[]).map((t) => (
              <button key={t} onClick={() => setTab(t)} className={`px-3 py-1.5 text-xs rounded ${tab === t ? "bg-ink-900 text-white" : "text-ink-500 hover:bg-ink-100"}`}>{tabLabels[t]}</button>
            ))}
          </div>
          {!project ? <div className="flex-1 flex items-center justify-center text-sm text-ink-400">请先加载项目</div> : <ChatPanel tab={tab} messages={messages} discussMsgs={discussMsgs} discussing={discussing} planInfo={planInfo} onOption={(key, supplement) => handleSend(supplement ? `${key}\n${supplement}` : key)} onSend={handleSend} onDiscuss={handleDiscuss} endRef={chatEndRef} />}
        </section>
      </div>

      <DeptStatusBar />
      <LogBar expanded={logsExpanded} onExpandedChange={setLogsExpanded} />
      {showPicker && <ProjectPicker recentDirs={recentDirs} pickerPath={pickerPath} pickerError={pickerError} pickerLoading={pickerLoading} setPickerPath={setPickerPath} onBrowse={handleBrowse} onLoad={handleLoadProject} onClose={() => setShowPicker(false)} />}
    </div>
  );
}
