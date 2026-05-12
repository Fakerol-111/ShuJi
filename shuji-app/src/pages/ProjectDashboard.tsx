import { useState, useEffect, useRef } from "react";
import { useNavigate } from "react-router-dom";
import { listen } from "@tauri-apps/api/event";
import { getProject, sendMessage, discussWithCabinet, getSnapshot, getTokenStats, getChatHistory } from "../api";
import type { TokenUsage } from "../api";
import type { Project, ChatMessage, ProjectSnapshot } from "../types";
import WorkflowTimeline from "../components/WorkflowTimeline";
import ChatBubble from "../components/ChatBubble";
import ChatInput from "../components/ChatInput";
import DeptStatusPanel from "../components/DeptStatusPanel";

const ROLE_NAMES: Record<string, string> = {
  zhongshu: "中书省", menxia: "门下省", neige: "内阁",
  shangshu: "尚书省", libup: "吏部", bingbu: "兵部",
  gongbu: "工部", xingbu: "刑部", libur: "礼部",
  hubu: "户部", zhisi: "制司",
};

const STORAGE_KEY = "shuji_chat";

function saveSession(msgs: ChatMessage[], discuss: ChatMessage[]) {
  try { sessionStorage.setItem(STORAGE_KEY, JSON.stringify({ msgs, discuss })); } catch {}
}

function loadSession(): { msgs: ChatMessage[]; discuss: ChatMessage[] } | null {
  try {
    const raw = sessionStorage.getItem(STORAGE_KEY);
    return raw ? JSON.parse(raw) : null;
  } catch { return null; }
}

type Tab = "decision" | "discuss";

export default function ProjectDashboard() {
  const navigate = useNavigate();
  const [project, setProject] = useState<Project | null>(null);
  const [snapshot, setSnapshot] = useState<ProjectSnapshot | null>(null);

  const session = loadSession();
  const [messages, setMessages] = useState<ChatMessage[]>(session?.msgs || []);
  // sending state removed — actor system always available

  const [discussMsgs, setDiscussMsgs] = useState<ChatMessage[]>(session?.discuss || [
    { role: "内阁", content: "想讨论什么？我随时可以聊。", options: [], documents: [], timestamp: new Date().toISOString() },
  ]);
  const [discussing, setDiscussing] = useState(false);

  const [tokenStats, setTokenStats] = useState<Record<string, Record<string, TokenUsage>> | null>(null);
  const [tokenWindow, setTokenWindow] = useState("汇总");
  const [showDashboard, setShowDashboard] = useState(false);
  const [error, setError] = useState("");
  const [tab, setTab] = useState<Tab>("decision");
  const chatEndRef = useRef<HTMLDivElement>(null);

  // Persist messages to sessionStorage on every change
  useEffect(() => { saveSession(messages, discussMsgs); }, [messages, discussMsgs]);

  useEffect(() => {
    getProject().then((p) => {
      if (!p) { navigate("/"); return; }
      setProject(p);
      if (!session) {
        setMessages([{
          role: "内阁",
          content: "有什么需要做的？请告诉我。",
          options: [], documents: [], timestamp: new Date().toISOString(),
        }]);
      }
      refreshSnapshot();
    }).catch(() => navigate("/"));
  }, []);

  useEffect(() => {
    chatEndRef.current?.scrollIntoView({ behavior: "smooth" });
  }, [messages, discussMsgs]);

  // Sync chat history on mount (recovers events missed during navigation)
  useEffect(() => {
    getChatHistory().then((hist) => {
      if (hist.length > 0) {
        setMessages((prev) => {
          // Deduplicate by timestamp + role: only append entries not in current list
          const existing = new Set(prev.map((m) => `${m.timestamp}|${m.role}|${m.content.slice(0, 40)}`));
          const newMsgs = hist.filter((m) => !existing.has(`${m.timestamp}|${m.role}|${m.content.slice(0, 40)}`));
          return newMsgs.length > 0 ? [...prev, ...newMsgs] : prev;
        });
      }
    }).catch(() => {});
  }, []);

  // Listen for real-time chat-message events from the engine (streamed during processing)
  useEffect(() => {
    const unlisten = listen<ChatMessage>("chat-message", (event) => {
      setMessages((prev) => [...prev, event.payload]);
    });
    return () => { unlisten.then((f) => f()); };
  }, []);

  const refreshSnapshot = async () => {
    try {
      const s = await getSnapshot();
      setSnapshot(s);
      const p = await getProject();
      setProject(p);
    } catch { /* ignore */ }
  };

  const handleSend = async (text: string) => {
    setError("");
    setMessages((prev) => [...prev, {
      role: "皇帝", content: text, options: [], documents: [], timestamp: new Date().toISOString(),
    }]);
    try {
      await sendMessage(text);
    } catch (e) {
      setError(String(e));
    }
  };

  const handleOption = async (key: string, supplement?: string) => {
    const text = supplement ? `${key}\n${supplement}` : key;
    await handleSend(text);
  };

  const handleDiscuss = async (text: string) => {
    setDiscussing(true);
    setDiscussMsgs((prev) => [...prev, {
      role: "皇帝", content: text, options: [], documents: [], timestamp: new Date().toISOString(),
    }]);
    try {
      const reply = await discussWithCabinet(text);
      setDiscussMsgs((prev) => [...prev, reply]);
    } catch (e) {
      setDiscussMsgs((prev) => [...prev, {
        role: "内阁", content: `讨论出错：${e}`, options: [], documents: [], timestamp: new Date().toISOString(),
      }]);
    } finally {
      setDiscussing(false);
    }
  };

  const maxTotal = tokenStats && tokenStats[tokenWindow] ? Math.max(...Object.values(tokenStats[tokenWindow]).map((u) => u.total_tokens), 1) : 1;

  return (
    <div className="h-screen bg-gray-50 flex flex-col overflow-hidden">
      {/* Header */}
      <header className="bg-white border-b shadow-sm shrink-0">
        <div className="max-w-6xl mx-auto px-6 py-3 flex items-center justify-between">
          <div className="flex items-center gap-3">
            <h1 className="text-lg font-bold text-gray-900">{project?.name || "枢机"}</h1>
            <span className="text-xs text-gray-400">{project?.working_dir}</span>
          </div>
          <div className="flex items-center gap-2">
            <button onClick={() => navigate("/")} className="text-sm text-gray-500 hover:text-gray-700">← 返回</button>
            <button onClick={() => navigate("/logs")} className="text-sm px-3 py-1.5 border border-gray-300 rounded hover:bg-gray-50">日志</button>
            <button onClick={async () => {
              try {
                if (!showDashboard) {
                  setTokenStats(await getTokenStats());
                }
                setShowDashboard(!showDashboard);
              } catch { /* ignore */ }
            }} className="text-sm px-3 py-1.5 border border-gray-300 rounded hover:bg-gray-50">仪表盘</button>
          </div>
        </div>

        <div className="max-w-6xl mx-auto px-6 flex gap-0">
          <TabButton active={tab === "decision"} onClick={() => setTab("decision")}>决策</TabButton>
          <TabButton active={tab === "discuss"} onClick={() => setTab("discuss")}>讨论</TabButton>
        </div>
      </header>

      {error && (
        <div className="max-w-6xl mx-auto w-full px-6 pt-2 shrink-0">
          <div className="bg-red-50 border border-red-300 text-red-800 px-4 py-2 rounded-lg text-sm">
            {error}
            <button onClick={() => setError("")} className="ml-2 text-red-500 hover:text-red-700 font-bold">&times;</button>
          </div>
        </div>
      )}

      {/* Main content — fixed height, internal scroll */}
      <div className="flex-1 max-w-6xl mx-auto w-full px-6 py-4 grid grid-cols-3 gap-6 min-h-0 overflow-hidden">
        <div className="flex flex-col gap-3 min-h-0 overflow-hidden">
          <div className="overflow-y-auto shrink-0">
            {snapshot && <WorkflowTimeline overallProgress={snapshot.overall_progress} phases={snapshot.phases} />}
          </div>
          <div className="flex-1 overflow-y-auto min-h-0">
            <DeptStatusPanel />
          </div>
        </div>

        <div className="col-span-2 bg-white rounded-lg border shadow-sm flex flex-col min-h-0">
          {tab === "decision" ? (
            <>
              <div className="flex-1 overflow-y-auto p-4 space-y-1">
                {messages.map((msg, i) => (
                  <ChatBubble key={i} msg={msg} onOption={handleOption} />
                ))}
                <div ref={chatEndRef} />
              </div>
              <ChatInput onSend={handleSend} disabled={false} placeholder="输入指令..." />
            </>
          ) : (
            <>
              <div className="flex-1 overflow-y-auto p-4 space-y-1">
                {discussMsgs.map((msg, i) => (
                  <ChatBubble key={i} msg={msg} onOption={() => {}} />
                ))}
                {discussing && <div className="flex items-center justify-center gap-3 py-2">
                  <span className="text-xs text-gray-400">内阁思考中...</span>
                  <span className="text-xs text-gray-400">（讨论不可取消，请等待）</span>
                </div>}
                <div ref={chatEndRef} />
              </div>
              <ChatInput onSend={handleDiscuss} disabled={discussing} placeholder="与内阁讨论..." />
            </>
          )}
        </div>
      </div>

      {/* Dashboard sidebar overlay */}
      {showDashboard && (
        <div className="fixed inset-y-0 right-0 w-96 bg-white shadow-2xl border-l z-50 flex flex-col">
          <div className="flex items-center justify-between px-5 py-4 border-b bg-gray-50 shrink-0">
            <h3 className="text-sm font-bold text-gray-700">Token 消耗仪表盘</h3>
            <div className="flex items-center gap-2">
              <button onClick={async () => setTokenStats(await getTokenStats())} className="text-xs text-gray-500 hover:text-gray-700">刷新</button>
              <button onClick={() => setShowDashboard(false)} className="text-gray-400 hover:text-gray-600 text-lg leading-none">&times;</button>
            </div>
          </div>
          <div className="flex-1 overflow-y-auto p-5">
            {/* Time window selector */}
            {tokenStats && Object.keys(tokenStats).length > 0 && (
              <div className="flex gap-1 mb-3 flex-wrap">
                {Object.keys(tokenStats).map((w) => (
                  <button
                    key={w}
                    onClick={() => setTokenWindow(w)}
                    className={`text-xs px-2 py-1 rounded ${tokenWindow === w ? "bg-blue-600 text-white" : "bg-gray-100 text-gray-600 hover:bg-gray-200"}`}
                  >
                    {w}
                  </button>
                ))}
              </div>
            )}
            {!tokenStats || Object.keys(tokenStats).length === 0 ? (
              <p className="text-xs text-gray-500">暂无数据</p>
            ) : (
              <div className="space-y-5">
                {Object.entries(tokenStats[tokenWindow] || {}).map(([role, usage]) => {
                  const pct = (usage.total_tokens / maxTotal) * 100;
                  return (
                    <div key={role}>
                      <div className="flex justify-between text-xs mb-1">
                        <span className="font-medium text-gray-700">{ROLE_NAMES[role] || role}</span>
                        <span className="text-gray-500">{usage.total_tokens.toLocaleString()} tokens</span>
                      </div>
                      <div className="w-full bg-gray-100 rounded-full h-3 overflow-hidden">
                        <div
                          className="h-full rounded-full transition-all duration-500"
                          style={{ width: `${Math.max(pct, 2)}%`, background: BAR_COLOR(role) }}
                        />
                      </div>
                      <div className="flex justify-between text-[10px] text-gray-400 mt-0.5">
                        <span>调用 {usage.call_count} 次</span>
                        <span>输入 {usage.prompt_tokens.toLocaleString()} / 输出 {usage.completion_tokens.toLocaleString()}</span>
                      </div>
                    </div>
                  );
                })}
              </div>
            )}
          </div>
        </div>
      )}
    </div>
  );
}

function TabButton({ active, onClick, children }: { active: boolean; onClick: () => void; children: React.ReactNode }) {
  return (
    <button
      onClick={onClick}
      className={`px-5 py-2 text-sm font-bold border-b-2 transition ${
        active
          ? "border-blue-600 text-blue-700"
          : "border-transparent text-gray-500 hover:text-gray-700 hover:border-gray-300"
      }`}
    >
      {children}
    </button>
  );
}

const BAR_COLOR = (() => {
  const palette = ["#3b82f6", "#10b981", "#f59e0b", "#ef4444", "#8b5cf6", "#06b6d4", "#ec4899", "#84cc16", "#14b8a6", "#f97316", "#6366f1"];
  const map: Record<string, string> = {};
  let i = 0;
  return (role: string) => {
    if (!map[role]) { map[role] = palette[i % palette.length]; i++; }
    return map[role];
  };
})();
