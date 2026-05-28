import ChatBubble from "./ChatBubble";
import ChatInput from "./ChatInput";
import type { ChatMessage, PlanInfo } from "../types";

interface ChatPanelProps {
  tab: "decision" | "discuss";
  messages: ChatMessage[];
  discussMsgs: ChatMessage[];
  discussing: boolean;
  planInfo: PlanInfo | null;
  onOption: (key: string, supplement?: string) => void;
  onSend: (text: string) => void;
  onDiscuss: (text: string) => void;
  endRef: React.RefObject<HTMLDivElement | null>;
}

export default function ChatPanel(props: ChatPanelProps) {
  const { tab, messages, discussMsgs, discussing, planInfo, onOption, onSend, onDiscuss, endRef } = props;
  if (tab === "decision") {
    return (
      <>
        {planInfo && <PlanCard info={planInfo} />}
        <MessageList messages={messages} onOption={onOption} endRef={endRef} />
        <ChatInput onSend={onSend} disabled={false} placeholder="输入指令..." />
      </>
    );
  }
  return (
    <>
      <MessageList messages={discussMsgs} onOption={() => {}} endRef={endRef} thinking={discussing} />
      <ChatInput onSend={onDiscuss} disabled={discussing} placeholder="与内阁讨论..." />
    </>
  );
}

function MessageList({ messages, onOption, endRef, thinking }: { messages: ChatMessage[]; onOption: (key: string, supplement?: string) => void; endRef: React.RefObject<HTMLDivElement | null>; thinking?: boolean }) {
  return (
    <div className="flex-1 overflow-y-auto p-4 space-y-2 min-h-0">
      {messages.map((msg, i) => <ChatBubble key={messageKey(msg, i)} msg={msg} onOption={onOption} />)}
      {thinking && <div className="flex items-center justify-center gap-3 py-2"><span className="text-xs text-ink-500">内阁思考中...</span></div>}
      <div ref={endRef} />
    </div>
  );
}

function messageKey(msg: ChatMessage, index: number) {
  return `${msg.timestamp || index}|${msg.role}|${msg.content.slice(0, 40)}`;
}

function PlanCard({ info }: { info: PlanInfo }) {
  return (
    <div className="shrink-0 mx-4 mt-3 bg-ink-100 border border-ink-200 rounded-lg px-3 py-2">
      <div className="text-[10px] text-ink-400 font-medium tracking-wide mb-1">工部计划</div>
      <div className="space-y-0.5">
        {info.batches.map((b, i) => (
          <div key={i} className="flex items-center gap-1.5 text-[11px] font-mono">
            <span className={`w-1.5 h-1.5 rounded-full shrink-0 ${b.status === "done" ? "bg-green-500" : b.status === "current" ? "bg-yellow-500 animate-pulse" : "bg-ink-300"}`} />
            <span className={b.status === "done" ? "text-ink-400 line-through" : b.status === "current" ? "text-ink-800 font-medium" : "text-ink-500"}>{b.name}</span>
            {b.status === "current" && <span className="text-ink-400 text-[10px] ml-auto truncate">{b.goal}</span>}
          </div>
        ))}
      </div>
    </div>
  );
}
