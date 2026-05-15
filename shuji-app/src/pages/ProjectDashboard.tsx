import { useState, useEffect, useRef } from "react";
import { useNavigate } from "react-router-dom";
import { listen } from "@tauri-apps/api/event";
import { getProject, sendMessage, discussWithCabinet, getTokenStats, getChatHistory } from "../api";
import type { TokenUsage } from "../api";
import type { Project, ChatMessage, PlanInfo } from "../types";
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

  const session = loadSession();
  const [messages, setMessages] = useState<ChatMessage[]>(session?.msgs || []);

  const [discussMsgs, setDiscussMsgs] = useState<ChatMessage[]>(session?.discuss || [
    { role: "内阁", content: "想讨论什么？我随时可以聊。", options: [], documents: [], timestamp: new Date().toISOString() },
  ]);
  const [discussing, setDiscussing] = useState(false);

  const [tokenStats, setTokenStats] = useState<Record<string, Record<string, TokenUsage>> | null>(null);
  const [tokenWindow, setTokenWindow] = useState("汇总");
  const [showDashboard, setShowDashboard] = useState(false);
  const [planInfo, setPlanInfo] = useState<PlanInfo | null>(null);
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
          const existing = new Set(prev.map((m) => `${m.timestamp}|${m.role}|${m.content.slice(0, 40)}`));
          const newMsgs = hist.filter((m) => !existing.has(`${m.timestamp}|${m.role}|${m.content.slice(0, 40)}`));
          return newMsgs.length > 0 ? [...prev, ...newMsgs] : prev;
        });
      }
    }).catch(() => {});
  }, []);

  // Listen for real-time chat-message events (only 内阁 now)
  useEffect(() => {
    const unlisten = listen<ChatMessage>("chat-message", (event) => {
      setMessages((prev) => [...prev, event.payload]);
    });
    return () => { unlisten.then((f) => f()); };
  }, []);

  // Listen for plan-update events (工部 batch progress)
  useEffect(() => {
    const unlisten = listen<PlanInfo>("plan-update", (event) => {
      if (event.payload.complete) {
        setPlanInfo(null);
      } else {
        setPlanInfo(event.payload);
      }
    });
    return () => { unlisten.then((f) => f()); };
  }, []);

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
    <div className="h-screen bg-ink-50 flex flex-col overflow-hidden">
      {/* Header */}
      <header className="bg-ink-900 border-b border-ink-800 shrink-0">
        <div className="px-5 py-2.5 flex items-center justify-between">
          <div className="flex items-center gap-3">
            <h1 className="text-base font-bold text-ink-50 tracking-wide">{project?.name || "枢机"}</h1>
            <span className="text-xs text-ink-500 font-mono">{project?.working_dir}</span>
          </div>
          <div className="flex items-center gap-1.5">
            <button onClick={() => navigate("/")} className="text-xs px-2.5 py-1.5 text-ink-400 hover:text-ink-200 hover:bg-ink-800 rounded transition-colors">← 返回</button>
            <button onClick={() => navigate("/logs")} className="text-xs px-2.5 py-1.5 text-ink-400 hover:text-ink-200 hover:bg-ink-800 rounded transition-colors">日志</button>
            <button onClick={async () => {
              try {
                if (!showDashboard) setTokenStats(await getTokenStats());
                setShowDashboard(!showDashboard);
              } catch { /* ignore */ }
            }} className="text-xs px-2.5 py-1.5 text-ink-400 hover:text-ink-200 hover:bg-ink-800 rounded transition-colors">度支</button>
          </div>
        </div>

        <div className="px-5 flex gap-0 border-t border-ink-800">
          <TabButton active={tab === "decision"} onClick={() => setTab("decision")}>决策</TabButton>
          <TabButton active={tab === "discuss"} onClick={() => setTab("discuss")}>讨论</TabButton>
        </div>
      </header>

      {error && (
        <div className="w-full px-5 pt-2 shrink-0">
          <div className="bg-vermillion-light border border-vermillion/20 text-vermillion-dark px-4 py-2 rounded text-sm">
            {error}
            <button onClick={() => setError("")} className="ml-2 text-vermillion hover:text-vermillion-dark font-bold">&times;</button>
          </div>
        </div>
      )}

      {/* Main content — left log panel + right chat */}
      <div className="flex-1 flex min-h-0 overflow-hidden">
        {/* Left: Log panel */}
        <div className="w-[35%] min-w-[300px] max-w-[450px] border-r border-ink-200 flex flex-col shrink-0">
          <DeptStatusPanel />
        </div>

        {/* Right: Chat */}
        <div className="flex-1 bg-ink-50 flex flex-col min-h-0 min-w-0">
          {tab === "decision" ? (
            <>
              {planInfo && <PlanCard info={planInfo} />}
              <div className="flex-1 overflow-y-auto p-4 space-y-2">
                {messages.map((msg, i) => (
                  <ChatBubble key={i} msg={msg} onOption={handleOption} />
                ))}
                <div ref={chatEndRef} />
              </div>
              <ChatInput onSend={handleSend} disabled={false} placeholder="输入指令..." />
            </>
          ) : (
            <>
              <div className="flex-1 overflow-y-auto p-4 space-y-2">
                {discussMsgs.map((msg, i) => (
                  <ChatBubble key={i} msg={msg} onOption={() => {}} />
                ))}
                {discussing && <div className="flex items-center justify-center gap-3 py-2">
                  <span className="text-xs text-ink-500">内阁思考中...</span>
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
        <div className="fixed inset-y-0 right-0 w-96 bg-ink-50 shadow-2xl border-l border-ink-200 z-50 flex flex-col">
          <div className="flex items-center justify-between px-5 py-3 border-b border-ink-200 bg-ink-100 shrink-0">
            <h3 className="text-sm font-bold text-ink-800">度支</h3>
            <div className="flex items-center gap-2">
              <button onClick={async () => setTokenStats(await getTokenStats())} className="text-xs text-ink-500 hover:text-ink-700">刷新</button>
              <button onClick={() => setShowDashboard(false)} className="text-ink-500 hover:text-ink-800 text-lg leading-none">&times;</button>
            </div>
          </div>
          <div className="flex-1 overflow-y-auto p-5">
            {tokenStats && Object.keys(tokenStats).length > 0 && (
              <div className="flex gap-1 mb-3 flex-wrap">
                {["今日", "近3日", "近7日", "汇总"].filter(w => tokenStats[w]).map((w) => (
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
              <p className="text-xs text-ink-500">暂无数据</p>
            ) : (
              <div className="space-y-4">
                {Object.entries(tokenStats[tokenWindow] || {})
                  .sort(([a], [b]) => {
                    const order = ["内阁", "中书令", "门下侍中", "尚书令", "吏部", "兵部", "工部", "礼部", "刑部"];
                    return order.indexOf(a) - order.indexOf(b);
                  })
                  .map(([role, usage]) => {
                  const pct = (usage.total_tokens / maxTotal) * 100;
                  return (
                    <div key={role}>
                      <div className="flex justify-between text-xs mb-1">
                        <span className="font-medium text-ink-700">{ROLE_NAMES[role] || role}</span>
                        <span className="text-ink-500">{usage.total_tokens.toLocaleString()} tokens</span>
                      </div>
                      <div className="w-full bg-ink-200 rounded-full h-2 overflow-hidden">
                        <div
                          className="h-full rounded-full transition-all duration-500"
                          style={{ width: `${Math.max(pct, 2)}%`, background: BAR_COLOR(role) }}
                        />
                      </div>
                      <div className="flex justify-between text-[10px] text-ink-400 mt-0.5">
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
      className={`px-4 py-2 text-xs font-medium border-b-2 transition-colors ${
        active
          ? "border-vermillion text-ink-50"
          : "border-transparent text-ink-500 hover:text-ink-300 hover:border-ink-600"
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

function PlanCard({ info }: { info: PlanInfo }) {
  return (
    <div className="shrink-0 mx-4 mb-1 bg-ink-100 border border-ink-200 rounded-lg px-3 py-2">
      <div className="text-[10px] text-ink-400 font-medium tracking-wide mb-1">工部计划</div>
      <div className="space-y-0.5">
        {info.batches.map((b, i) => (
          <div key={i} className="flex items-center gap-1.5 text-[11px] font-mono">
            <span className={`w-1.5 h-1.5 rounded-full shrink-0 ${
              b.status === "done" ? "bg-green-500" :
              b.status === "current" ? "bg-yellow-500 animate-pulse" :
              "bg-ink-300"
            }`} />
            <span className={
              b.status === "done" ? "text-ink-400 line-through" :
              b.status === "current" ? "text-ink-800 font-medium" :
              "text-ink-500"
            }>
              {b.name}
            </span>
            {b.status === "current" && (
              <span className="text-ink-400 text-[10px] ml-auto">{b.goal}</span>
            )}
          </div>
        ))}
      </div>
    </div>
  );
}
