import { useState, useEffect, useRef, useCallback } from "react";
import { listen } from "@tauri-apps/api/event";
import { sendMessage, discussWithCabinet, getChatHistory } from "../api";
import { formatError } from "../utils/error";
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
    }).catch((e) => setError(`读取聊天历史失败：${formatError(e)}`));
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
    const ts = new Date().toISOString();
    setError("");
    const msg: ChatMessage = { role: "皇帝", content: text, options: [], documents: [], timestamp: ts };
    setMessages((prev) => [...prev, msg]);
    try { await sendMessage(text); } catch (e) {
      setError(formatError(e));
      // Mark the optimistic message as failed
      setMessages((prev) => prev.map((m) => m.timestamp === ts && m.role === "皇帝" ? { ...m, status: "failed" } : m));
    }
  };

  const retrySend = async (text: string, originalTs: string) => {
    // Remove the failed message, then re-send
    setMessages((prev) => prev.filter((m) => m.timestamp !== originalTs || m.role !== "皇帝"));
    await handleSend(text);
  };

  // ── Discuss cancellation support ──
  const discussCancelRef = useRef(false);

  const handleDiscuss = async (text: string) => {
    discussCancelRef.current = false;
    setDiscussing(true);
    setDiscussMsgs((prev) => [...prev, { role: "皇帝", content: text, options: [], documents: [], timestamp: new Date().toISOString() }]);
    try {
      const reply = await discussWithCabinet(text);
      // If cancelled while in flight, ignore the result
      if (discussCancelRef.current) {
        setDiscussMsgs((prev) => [...prev, initialCabinetMessage("讨论已取消。")]);
        return;
      }
      setDiscussMsgs((prev) => [...prev, reply]);
    } catch (e) { setDiscussMsgs((prev) => [...prev, initialCabinetMessage(`讨论出错：${formatError(e)}`)]); }
    finally { setDiscussing(false); discussCancelRef.current = false; }
  };

  const cancelDiscuss = useCallback(() => {
    if (discussing) {
      discussCancelRef.current = true;
      setDiscussing(false);
      setDiscussMsgs((prev) => [...prev, initialCabinetMessage("讨论已取消。")]);
    }
  }, [discussing]);

  const resetDiscuss = () => setDiscussMsgs([initialCabinetMessage("想讨论什么？我随时可以聊。")]);

  return { messages, discussMsgs, discussing, tab, planInfo, error, setError, setTab, setMessages, handleSend, retrySend, handleDiscuss, cancelDiscuss, resetDiscuss, chatEndRef };
}
