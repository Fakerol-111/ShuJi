import { useState, useRef, useEffect, useCallback } from "react";

interface Props {
  onSend: (msg: string) => void;
  disabled?: boolean;
  placeholder?: string;
}

const LINE_HEIGHT = 20; // matches text-sm leading-5
const MAX_LINES = 4;

export default function ChatInput({ onSend, disabled, placeholder }: Props) {
  const [text, setText] = useState("");
  const textareaRef = useRef<HTMLTextAreaElement>(null);

  useEffect(() => {
    textareaRef.current?.focus();
  }, []);

  const adjustHeight = useCallback(() => {
    const el = textareaRef.current;
    if (!el) return;
    el.style.height = "auto";
    const maxH = LINE_HEIGHT * MAX_LINES;
    el.style.height = `${Math.min(el.scrollHeight, maxH)}px`;
  }, []);

  useEffect(() => {
    adjustHeight();
  }, [text, adjustHeight]);

  const handleSend = () => {
    const trimmed = text.trim();
    if (!trimmed || disabled) return;
    onSend(trimmed);
    setText("");
  };

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === "Enter" && !e.shiftKey) {
      e.preventDefault();
      handleSend();
    }
  };

  return (
    <div className="flex gap-2 border-t border-ink-200 bg-white px-4 py-3 items-end">
      <textarea
        ref={textareaRef}
        value={text}
        onChange={(e) => setText(e.target.value)}
        onKeyDown={handleKeyDown}
        placeholder={placeholder || "输入消息..."}
        disabled={disabled}
        rows={1}
        className="flex-1 px-3 py-2 border border-ink-200 bg-ink-50 rounded-lg text-sm text-ink-900 placeholder:text-ink-400 focus:outline-none focus:border-vermillion focus:ring-1 focus:ring-vermillion/30 disabled:opacity-50 resize-none leading-5"
      />
      <button
        onClick={handleSend}
        disabled={disabled || !text.trim()}
        className="px-5 py-2 bg-ink-900 text-ink-50 rounded-lg hover:bg-ink-800 disabled:opacity-40 text-sm font-medium transition-colors shrink-0"
      >
        发送
      </button>
    </div>
  );
}
