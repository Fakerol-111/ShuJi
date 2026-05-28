import { useState, useEffect, useRef } from "react";
import { listen } from "@tauri-apps/api/event";
import { sendMessage, discussWithCabinet, getChatHistory } from "../api";
import type { ChatMessage, PlanInfo } from "../types";

function initialCabinetMessage(content: string): ChatMessage {
  return { role: "内阁", content, options: [], documents: [], timestamp: new Date().toISOString() };
}

function mergeMessages(prev: ChatMessage[], hist: ChatMessage[]) {
  const existing = new Set(prev.map((m) => `${m.timestamp}|${m.role}|${m.content.slice(0, 40)}`));
  const newMsgs = hist.filter((m) => !existing.has(`${m.timestamp}|${m.role}|${m.content.slice(0, 40)}`));
  return newMsgs.length > 0 ? [...prev, ...newMsgs] : prev;
}

export type Tab = "decision" | "discuss";

export function useChat(initialMessages: ChatMessage[]) {
  const [messages, setMessages] = useState<ChatMessage[]>(initialMessages);
  const [discussMsgs, setDiscussMsgs] = useState<ChatMessage[]>([initialCabinetMessage("想讨论什么？我随时可以聊。")]);
  const [discussing, setDiscussing] = useState(false);
  const [tab, setTab] = useState<Tab>("decision");
  const [planInfo, setPlanInfo] = useState<PlanInfo | null>(null);
  const [error, setError] = useState("");
  const chatEndRef = useRef<HTMLDivElement>(null);

  // Scroll to end on new messages
  useEffect(() => { chatEndRef.current?.scrollIntoView({ behavior: "smooth" }); }, [messages, discussMsgs, tab]);

  // Restore chat history
  useEffect(() => {
    getChatHistory().then((hist) => {
      if (hist.length > 0) setMessages((prev) => mergeMessages(prev, hist));
    }).catch((e) => setError(`读取聊天历史失败：${e}`));
  }, []);

  // Listen for real-time chat messages
  useEffect(() => {
    const unlisten = listen<ChatMessage>("chat-message", (event) => setMessages((prev) => [...prev, event.payload]));
    return () => { unlisten.then((f) => f()); };
  }, []);

  // Listen for plan updates
  useEffect(() => {
    const unlisten = listen<PlanInfo>("plan-update", (event) => setPlanInfo(event.payload.complete ? null : event.payload));
    return () => { unlisten.then((f) => f()); };
  }, []);

  const handleSend = async (text: string) => {
    setError("");
    setMessages((prev) => [...prev, { role: "皇帝", content: text, options: [], documents: [], timestamp: new Date().toISOString() }]);
    try { await sendMessage(text); } catch (e) { setError(String(e)); }
  };

  const handleDiscuss = async (text: string) => {
    setDiscussing(true);
    setDiscussMsgs((prev) => [...prev, { role: "皇帝", content: text, options: [], documents: [], timestamp: new Date().toISOString() }]);
    try {
      const reply = await discussWithCabinet(text);
      setDiscussMsgs((prev) => [...prev, reply]);
    } catch (e) { setDiscussMsgs((prev) => [...prev, initialCabinetMessage(`讨论出错：${e}`)]); }
    finally { setDiscussing(false); }
  };

  const resetDiscuss = () => setDiscussMsgs([initialCabinetMessage("想讨论什么？我随时可以聊。")]);

  return { messages, discussMsgs, discussing, tab, planInfo, error, setError, setTab, setMessages, handleSend, handleDiscuss, resetDiscuss, chatEndRef };
}
