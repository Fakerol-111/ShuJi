import { useState } from "react";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import rehypeHighlight from "rehype-highlight";
import type { ChatMessage, ChatOption } from "../types";
import { getDeptMeta } from "../constants";

export default function ChatBubble({ msg, onOption }: { msg: ChatMessage; onOption: (key: string, supplement?: string) => void }) {
  const isEmperor = msg.role === "皇帝";
  const deptColor = getDeptMeta(msg.role)?.color;

  return (
    <div className={`flex ${isEmperor ? "justify-end" : "justify-start"}`}>
      <div className={`${isEmperor ? "max-w-[70%]" : "max-w-[85%]"}`}>
        {/* Header */}
        <div className={`text-caption mb-1 ${isEmperor ? "text-right text-ink-500" : "text-ink-600"}`}>
          {isEmperor ? "御" : `${msg.role} 回奏`}
        </div>

        {/* Emperor bubble */}
        {isEmperor ? (
          <div className="bg-ink-900 text-ink-50 rounded-xl rounded-tr-sm px-4 py-2.5 text-body leading-relaxed">
            <p className="whitespace-pre-wrap break-words overflow-hidden">{msg.content}</p>
          </div>
        ) : (
          /* Department bubble: 3px left color bar */
          <div
            className="bg-surface-elevated border border-fold rounded-xl rounded-tl-sm px-4 py-2.5 text-body leading-relaxed"
            style={deptColor ? { borderLeft: `3px solid ${deptColor}` } : undefined}
          >
            <div className="prose prose-shuji max-w-none break-words">
              <ReactMarkdown
                remarkPlugins={[remarkGfm]}
                rehypePlugins={[rehypeHighlight]}
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
          </div>
        )}

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
      <div className="mt-2 bg-ink-100 border border-fold rounded-lg p-3">
        <p className="text-ui font-bold text-ink-800 mb-1">{opt.key}. {opt.label}</p>
        <p className="text-caption text-ink-600 mb-2">{opt.description}</p>
        <textarea
          className="w-full border border-fold rounded px-3 py-2 text-body resize-none bg-surface-elevated text-ink-900 focus:outline-none focus:border-vermillion"
          rows={3}
          placeholder="在此补充御批..."
          value={supplement}
          onChange={(e) => setSupplement(e.target.value)}
        />
        <div className="flex gap-2 mt-2">
          <button
            onClick={() => onOption(selectedKey, supplement)}
            className="bg-vermillion text-white text-ui font-bold px-4 py-1.5 rounded-lg hover:bg-vermillion-dark transition-colors"
          >
            遵旨
          </button>
          <button
            onClick={() => { setSelectedKey(null); setSupplement(""); }}
            className="text-ui text-ink-500 px-3 py-1.5 hover:text-ink-700"
          >
            作罢
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
          className={`text-ui font-bold px-3 py-1.5 rounded-lg transition-colors ${
            opt.key === "A" ? "bg-jade text-white hover:bg-jade/80" :
            opt.key === "B" ? "bg-vermillion text-white hover:bg-vermillion-dark" :
            opt.key === "C" ? "bg-ink-700 text-white hover:bg-ink-800" :
            `bg-ink-600 text-white hover:bg-ink-700`
          }`}
          title={opt.description}
        >
          {opt.key}. {opt.label}
        </button>
      ))}
    </div>
  );
}
