import { useState } from "react";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import type { ChatMessage, ChatOption } from "../types";

export default function ChatBubble({ msg, onOption }: { msg: ChatMessage; onOption: (key: string, supplement?: string) => void }) {
  const isEmperor = msg.role === "皇帝";

  return (
    <div className={`flex ${isEmperor ? "justify-end" : "justify-start"} mb-3`}>
      <div className={`max-w-[80%] ${isEmperor ? "order-1" : "order-1"}`}>
        <div className={`text-xs mb-0.5 ${isEmperor ? "text-right text-gray-400" : "text-gray-500"}`}>
          {isEmperor ? "皇帝" : msg.role}
        </div>
        <div
          className={`rounded-xl px-4 py-2.5 text-sm leading-relaxed ${
            isEmperor
              ? "bg-blue-600 text-white rounded-tr-sm"
              : "bg-gray-100 text-gray-800 rounded-tl-sm"
          }`}
        >
          {isEmperor ? (
            <p className="whitespace-pre-wrap">{msg.content}</p>
          ) : (
            <div className="prose prose-sm max-w-none prose-headings:text-gray-900 prose-a:text-blue-600 prose-code:bg-gray-200 prose-code:px-1 prose-code:rounded prose-pre:bg-gray-800 prose-pre:text-gray-100">
              <ReactMarkdown remarkPlugins={[remarkGfm]}>
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
    const opt = options.find((o) => o.key === selectedKey)!;
    return (
      <div className="mt-2 bg-blue-50 border border-blue-200 rounded-lg p-3">
        <p className="text-xs font-bold text-blue-800 mb-1">{opt.key}. {opt.label}</p>
        <p className="text-xs text-blue-600 mb-2">{opt.description}</p>
        <textarea
          className="w-full border border-blue-300 rounded px-3 py-2 text-sm resize-none"
          rows={3}
          placeholder="在此补充御批..."
          value={supplement}
          onChange={(e) => setSupplement(e.target.value)}
        />
        <div className="flex gap-2 mt-2">
          <button
            onClick={() => onOption(selectedKey, supplement)}
            className="bg-blue-600 text-white text-xs font-bold px-4 py-1.5 rounded-lg hover:bg-blue-700"
          >
            确认提交
          </button>
          <button
            onClick={() => { setSelectedKey(null); setSupplement(""); }}
            className="text-xs text-gray-500 px-3 py-1.5 hover:text-gray-700"
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
          className={`${BTN_COLORS[opt.key] || "bg-gray-600 hover:bg-gray-700"} text-white text-xs font-bold px-3 py-1.5 rounded-lg transition`}
          title={opt.description}
        >
          {opt.key}. {opt.label}
        </button>
      ))}
    </div>
  );
}

const BTN_COLORS: Record<string, string> = {
  A: "bg-green-600 hover:bg-green-700",
  B: "bg-blue-600 hover:bg-blue-700",
  C: "bg-red-600 hover:bg-red-700",
  D: "bg-yellow-600 hover:bg-yellow-700",
  E: "bg-purple-600 hover:bg-purple-700",
};
