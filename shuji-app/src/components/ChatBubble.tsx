import { useState } from "react";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import type { ChatMessage, ChatOption } from "../types";

export default function ChatBubble({ msg, onOption }: { msg: ChatMessage; onOption: (key: string, supplement?: string) => void }) {
  const isEmperor = msg.role === "皇帝";

  return (
    <div className={`flex ${isEmperor ? "justify-end" : "justify-start"} mb-3`}>
      <div className={`max-w-[80%]`}>
        <div className={`text-[10px] mb-1 tracking-wide ${isEmperor ? "text-right text-ink-500" : "text-ink-400"}`}>
          {isEmperor ? "陛下" : msg.role}
        </div>
        <div
          className={`rounded-xl px-4 py-2.5 text-sm leading-relaxed ${
            isEmperor
              ? "bg-ink-900 text-ink-50 rounded-tr-sm"
              : "bg-white border border-ink-200 text-ink-800 rounded-tl-sm"
          }`}
        >
          {isEmperor ? (
            <p className="whitespace-pre-wrap break-words overflow-hidden">{msg.content}</p>
          ) : (
            <div className="prose prose-sm max-w-none break-words prose-headings:text-ink-900 prose-a:text-vermillion">
              <ReactMarkdown
                remarkPlugins={[remarkGfm]}
                components={{
                  a: ({ href, children }) => (
                    <a href={href} target="_blank" rel="noopener noreferrer"
                       onClick={(e) => { e.preventDefault(); if (href) window.open(href, "_blank"); }}>
                      {children}
                    </a>
                  ),
                }}
              >
                {msg.content}
              </ReactMarkdown>
            </div>
          )}
        </div>
        {msg.options.length > 0 && (
          <OptionGroup options={msg.options} onOption={onOption} />
        )}
      </div>
    </div>
  );
}

function OptionGroup({ options, onOption }: { options: ChatOption[]; onOption: (key: string, supplement?: string) => void }) {
  const [selectedKey, setSelectedKey] = useState<string | null>(null);
  const [supplement, setSupplement] = useState("");

  if (selectedKey) {
    const opt = options.find((o) => o.key === selectedKey);
    if (!opt) return null;
    return (
      <div className="mt-2 bg-ink-100 border border-ink-200 rounded-lg p-3">
        <p className="text-xs font-bold text-ink-800 mb-1">{opt.key}. {opt.label}</p>
        <p className="text-xs text-ink-600 mb-2">{opt.description}</p>
        <textarea
          className="w-full border border-ink-300 rounded px-3 py-2 text-sm resize-none bg-white text-ink-900 focus:outline-none focus:border-vermillion"
          rows={3}
          placeholder="在此补充御批..."
          value={supplement}
          onChange={(e) => setSupplement(e.target.value)}
        />
        <div className="flex gap-2 mt-2">
          <button
            onClick={() => onOption(selectedKey, supplement)}
            className="bg-vermillion text-white text-xs font-bold px-4 py-1.5 rounded-lg hover:bg-vermillion-dark transition-colors"
          >
            确认
          </button>
          <button
            onClick={() => { setSelectedKey(null); setSupplement(""); }}
            className="text-xs text-ink-500 px-3 py-1.5 hover:text-ink-700"
          >
            取消
          </button>
        </div>
      </div>
    );
  }

  return (
    <div className="flex flex-wrap gap-1.5 mt-2">
      {options.map((opt) => (
        <button
          key={opt.key}
          onClick={() => {
            if (opt.label.includes("补充") || opt.key === "C") {
              setSelectedKey(opt.key);
            } else {
              onOption(opt.key);
            }
          }}
          className={`text-white text-xs font-bold px-3 py-1.5 rounded-lg transition-colors ${
            opt.key === "A" ? "bg-jade hover:bg-jade/80" :
            opt.key === "B" ? "bg-vermillion hover:bg-vermillion-dark" :
            opt.key === "C" ? "bg-ink-700 hover:bg-ink-800" :
            `bg-ink-600 hover:bg-ink-700`
          }`}
          title={opt.description}
        >
          {opt.key}. {opt.label}
        </button>
      ))}
    </div>
  );
}
