import type { ChatMessage, ChatOption } from "../types";

export default function ChatBubble({ msg, onOption }: { msg: ChatMessage; onOption: (key: string) => void }) {
  const isEmperor = msg.role === "皇帝";

  return (
    <div className={`flex ${isEmperor ? "justify-end" : "justify-start"} mb-3`}>
      <div className={`max-w-[80%] ${isEmperor ? "order-1" : "order-1"}`}>
        {/* Role label */}
        <div className={`text-xs mb-0.5 ${isEmperor ? "text-right text-gray-400" : "text-gray-500"}`}>
          {isEmperor ? "皇帝" : msg.role}
        </div>

        {/* Bubble */}
        <div
          className={`rounded-xl px-4 py-2.5 text-sm leading-relaxed whitespace-pre-wrap ${
            isEmperor
              ? "bg-blue-600 text-white rounded-tr-sm"
              : "bg-gray-100 text-gray-800 rounded-tl-sm"
          }`}
        >
          {msg.content}
        </div>

        {/* Options (inline buttons, only for 内阁 messages) */}
        {msg.options.length > 0 && (
          <div className="flex flex-wrap gap-1.5 mt-2">
            {msg.options.map((opt) => (
              <OptionButton key={opt.key} option={opt} onClick={() => onOption(opt.key)} />
            ))}
          </div>
        )}
      </div>
    </div>
  );
}

function OptionButton({ option, onClick }: { option: ChatOption; onClick: () => void }) {
  const colors: Record<string, string> = {
    A: "bg-green-600 hover:bg-green-700",
    B: "bg-blue-600 hover:bg-blue-700",
    C: "bg-red-600 hover:bg-red-700",
    D: "bg-yellow-600 hover:bg-yellow-700",
    E: "bg-purple-600 hover:bg-purple-700",
  };
  const color = colors[option.key] || "bg-gray-600 hover:bg-gray-700";

  return (
    <button
      onClick={onClick}
      className={`${color} text-white text-xs font-bold px-3 py-1.5 rounded-lg transition`}
      title={option.description}
    >
      {option.key}. {option.label}
    </button>
  );
}
