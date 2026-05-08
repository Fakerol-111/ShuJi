import { useState, useEffect, useRef } from "react";
import { useNavigate } from "react-router-dom";
import { getProject, sendMessage, getSnapshot, listLogFiles, readLogFile } from "../api";
import type { Project, ChatMessage, ProjectSnapshot } from "../types";
import WorkflowTimeline from "../components/WorkflowTimeline";
import ChatBubble from "../components/ChatBubble";
import ChatInput from "../components/ChatInput";

export default function ProjectDashboard() {
  const navigate = useNavigate();
  const [project, setProject] = useState<Project | null>(null);
  const [snapshot, setSnapshot] = useState<ProjectSnapshot | null>(null);
  const [messages, setMessages] = useState<ChatMessage[]>([]);
  const [sending, setSending] = useState(false);
  const [logFiles, setLogFiles] = useState<string[]>([]);
  const [selectedLog, setSelectedLog] = useState<string | null>(null);
  const [logContent, setLogContent] = useState<string[]>([]);
  const [showLogs, setShowLogs] = useState(false);
  const [error, setError] = useState("");
  const chatEndRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    getProject().then((p) => {
      if (!p) { navigate("/"); return; }
      setProject(p);
      // Show welcome message
      setMessages([{
        role: "内阁",
        content: "陛下有何吩咐？臣随时听命。",
        options: [],
        documents: [],
        timestamp: new Date().toISOString(),
      }]);
      refreshSnapshot();
    }).catch(() => navigate("/"));
  }, []);

  // Auto scroll to bottom on new messages
  useEffect(() => {
    chatEndRef.current?.scrollIntoView({ behavior: "smooth" });
  }, [messages]);

  const refreshSnapshot = async () => {
    try {
      const s = await getSnapshot();
      setSnapshot(s);
      const p = await getProject();
      setProject(p);
    } catch { /* ignore */ }
  };

  const handleSend = async (text: string) => {
    setSending(true);
    setError("");

    // Add emperor message to chat
    setMessages((prev) => [...prev, {
      role: "皇帝",
      content: text,
      options: [],
      documents: [],
      timestamp: new Date().toISOString(),
    }]);

    try {
      const response = await sendMessage(text);
      // Add response messages
      if (response.messages.length > 0) {
        setMessages((prev) => [...prev, ...response.messages]);
      }
      // Update snapshot
      setSnapshot(response.snapshot);
      const p = await getProject();
      setProject(p);
    } catch (e) {
      setError(String(e));
    } finally {
      setSending(false);
    }
  };

  const handleOption = async (key: string) => {
    await handleSend(key);
  };

  return (
    <div className="min-h-screen bg-gray-50 flex flex-col">
      {/* Header */}
      <header className="bg-white border-b shadow-sm">
        <div className="max-w-6xl mx-auto px-6 py-3 flex items-center justify-between">
          <div className="flex items-center gap-3">
            <h1 className="text-lg font-bold text-gray-900">
              {project?.name || "枢机"}
            </h1>
            <span className="text-xs text-gray-400">{project?.working_dir}</span>
          </div>
          <div className="flex items-center gap-2">
            <button onClick={() => navigate("/")} className="text-sm text-gray-500 hover:text-gray-700">
              ← 返回
            </button>
            <button
              onClick={async () => {
                try {
                  const files = await listLogFiles();
                  setLogFiles(files);
                  setSelectedLog(files.length > 0 ? files[0] : null);
                  if (files.length > 0) {
                    const content = await readLogFile(files[0]);
                    setLogContent(content);
                  }
                  setShowLogs(!showLogs);
                } catch { /* ignore */ }
              }}
              className="text-sm px-3 py-1.5 border border-gray-300 rounded hover:bg-gray-50"
            >
              日志
            </button>
          </div>
        </div>
      </header>

      {/* Error banner */}
      {error && (
        <div className="max-w-6xl mx-auto w-full px-6 pt-2">
          <div className="bg-red-50 border border-red-300 text-red-800 px-4 py-2 rounded-lg text-sm">
            {error}
            <button onClick={() => setError("")} className="ml-2 text-red-500 hover:text-red-700 font-bold">&times;</button>
          </div>
        </div>
      )}

      {/* Main content */}
      <div className="flex-1 max-w-6xl mx-auto w-full px-6 py-4 grid grid-cols-3 gap-6 min-h-0">
        {/* Left: Status panel */}
        <div className="space-y-4">
          {/* Project goal */}
          {project?.goal && (
            <div className="bg-white rounded-lg border p-3">
              <p className="text-xs text-gray-400 mb-1">皇帝目标</p>
              <p className="text-sm text-gray-800">{project.goal}</p>
            </div>
          )}

          {/* Workflow progress */}
          {snapshot && (
            <WorkflowTimeline
              overallProgress={snapshot.overall_progress}
              phases={snapshot.phases}
            />
          )}
        </div>

        {/* Right: Chat panel */}
        <div className="col-span-2 bg-white rounded-lg border shadow-sm flex flex-col min-h-[70vh]">
          <div className="flex-1 overflow-y-auto p-4 space-y-1">
            {messages.map((msg, i) => (
              <ChatBubble key={i} msg={msg} onOption={handleOption} />
            ))}
            {sending && (
              <div className="text-center text-xs text-gray-400 py-2">处理中...</div>
            )}
            <div ref={chatEndRef} />
          </div>
          <ChatInput onSend={handleSend} disabled={sending} placeholder="输入指令、目标或选择..." />
        </div>
      </div>

      {/* Log panel */}
      {showLogs && (
        <div className="max-w-6xl mx-auto w-full px-6 pb-4">
          <div className="bg-white rounded-lg border shadow-sm">
            <div className="flex items-center gap-2 border-b px-4 py-2 bg-gray-50">
              <h3 className="text-sm font-bold text-gray-700">日志</h3>
              <div className="flex gap-1 ml-2 overflow-x-auto">
                {logFiles.map((f) => {
                  const label = f.replace(".jsonl", "");
                  return (
                    <button
                      key={f}
                      onClick={async () => {
                        setSelectedLog(f);
                        const content = await readLogFile(f);
                        setLogContent(content);
                      }}
                      className={`text-xs px-2 py-1 rounded whitespace-nowrap ${
                        selectedLog === f
                          ? "bg-gray-800 text-white"
                          : "bg-gray-200 text-gray-700 hover:bg-gray-300"
                      }`}
                    >
                      {label}
                    </button>
                  );
                })}
              </div>
            </div>
            <div className="bg-gray-900 text-gray-100 p-4 max-h-64 overflow-y-auto">
              {logContent.length === 0 && (
                <p className="text-xs text-gray-500">暂无日志</p>
              )}
              {logContent.map((line, i) => (
                <pre key={i} className="text-xs leading-relaxed font-mono whitespace-pre-wrap">
                  {(() => {
                    try { return JSON.stringify(JSON.parse(line), null, 1); }
                    catch { return line; }
                  })()}
                </pre>
              ))}
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
